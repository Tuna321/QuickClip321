
$ErrorActionPreference='Stop'
$tx=$env:QC_UPD_TX
$target=$env:QC_UPD_TARGET
$version=$env:QC_UPD_VERSION
$channel=$env:QC_UPD_CHANNEL
$state=$env:QC_UPD_STATE
$oldPid=[int]$env:QC_UPD_PID
$token=$env:QC_UPD_TOKEN
$backup=Join-Path $tx 'previous.exe'
$stateBackup=Join-Path $tx 'previous-state.json'
$stateRecovery=$state+'.backup'
$stateRecoveryBackup=Join-Path $tx 'previous-state-backup.json'
$marker=Join-Path $tx 'healthy.txt'
$log=Join-Path $tx 'result.txt'
$changed=$false
$child=$null
$oldExited=$false
try{
 if([IO.Path]::GetDirectoryName($target) -ine [IO.Path]::GetDirectoryName($tx) -or [IO.Path]::GetFileName($tx) -notmatch '^\.quickclip-update-[a-zA-Z0-9]+$'){throw 'Invalid transaction directory'}
 if((Get-Item -LiteralPath $tx).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Unsafe transaction directory'}
 $old=Get-Process -Id $oldPid -ErrorAction SilentlyContinue
 if($old){
  if(!$old.HasExited){
   $oldPath=$null
   try{$oldPath=$old.Path}catch{}
   if(!$oldPath){
    if(!$old.WaitForExit(1000)){throw 'Cannot verify old process'}
   }elseif($oldPath -ine $target){
    if(!$old.HasExited){throw 'Unexpected old process'}
   }elseif(!$old.WaitForExit(30000)){throw 'Old process did not exit'}
  }
 }
 $oldExited=$true
 $m=Assert-UpdateManifest $tx $version $channel
 $artifact=Assert-UpdateArtifact $tx $m
 if(Test-Path -LiteralPath $state -PathType Leaf){[IO.File]::Copy($state,$stateBackup,$false)}
 if(Test-Path -LiteralPath $stateRecovery -PathType Leaf){
  if((Get-Item -LiteralPath $stateRecovery).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Unsafe state backup'}
  [IO.File]::Copy($stateRecovery,$stateRecoveryBackup,$false)
 }
 for($i=0;$i -lt 20;$i++){
  try{[IO.File]::Replace($artifact,$target,$backup,$true);$changed=$true;break}
  catch{if($i -eq 19){throw};Start-Sleep -Milliseconds 250}
 }
 $env:QC_UPDATE_HEALTH_PATH=$marker
 $env:QC_UPDATE_HEALTH_TOKEN=$token
 $env:QC_UPDATE_HEALTH_VERSION=$version
 $child=Start-Process -FilePath $target -PassThru
 Remove-Item Env:QC_UPDATE_HEALTH_PATH,Env:QC_UPDATE_HEALTH_TOKEN,Env:QC_UPDATE_HEALTH_VERSION -ErrorAction SilentlyContinue
 $watch=[Diagnostics.Stopwatch]::StartNew()
 $healthy=$false
 while($watch.Elapsed.TotalSeconds -lt 35){
  if($child.HasExited){break}
  if((Test-Path -LiteralPath $marker -PathType Leaf) -and [IO.File]::ReadAllText($marker) -ceq "$version|$token"){$healthy=$true;break}
  Start-Sleep -Milliseconds 250
 }
 if(!$healthy){throw 'New version did not report healthy UI'}
 [IO.File]::WriteAllText($log,'Installed; previous EXE and settings backup retained.')
}catch{
 Remove-Item Env:QC_UPDATE_HEALTH_PATH,Env:QC_UPDATE_HEALTH_TOKEN,Env:QC_UPDATE_HEALTH_VERSION -ErrorAction SilentlyContinue
 $reason=$_.Exception.Message
 try{
  if($changed){
   if($child -and !$child.HasExited){$child.Kill();if(!$child.WaitForExit(10000)){throw 'Could not stop failed update process'}}
   [IO.File]::Replace($backup,$target,(Join-Path $tx 'failed.exe'),$true)
   if(Test-Path -LiteralPath $stateBackup){
    if(Test-Path -LiteralPath $state){[IO.File]::Replace($stateBackup,$state,(Join-Path $tx 'failed-state.json'),$true)}
    else{[IO.File]::Move($stateBackup,$state)}
   }
   if(Test-Path -LiteralPath $stateRecoveryBackup){
    if(Test-Path -LiteralPath $stateRecovery){[IO.File]::Replace($stateRecoveryBackup,$stateRecovery,(Join-Path $tx 'failed-state-backup.json'),$true)}
    else{[IO.File]::Move($stateRecoveryBackup,$stateRecovery)}
   }elseif(Test-Path -LiteralPath $stateRecovery -PathType Leaf){
    [IO.File]::Move($stateRecovery,(Join-Path $tx 'failed-state-backup.json'))
   }
   [IO.File]::WriteAllText($log,"Rolled back: $reason")
   Start-Process -FilePath $target
  }elseif($oldExited){
   [IO.File]::WriteAllText($log,"Not installed: $reason")
   Start-Process -FilePath $target
  }else{[IO.File]::WriteAllText($log,"Not installed: $reason")}
 }catch{[IO.File]::WriteAllText($log,"Recovery required. Previous files retained. $reason / $($_.Exception.Message)")}
 exit 1
}
