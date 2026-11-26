param([Parameter(Mandatory=$true)][string]$StableExe,[Parameter(Mandatory=$true)][string]$NightlyExe,[Parameter(Mandatory=$true)][string]$Tools,[string]$StableVersion='3.0.0-beta.6',[string]$NightlyVersion='3.0.0-beta.6.nightly.3')
$ErrorActionPreference='Stop'
$root=Join-Path ([IO.Path]::GetTempPath()) ('QuickClip-switch-fixture-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $root|Out-Null
$public=[IO.File]::ReadAllText((Join-Path $PSScriptRoot 'desktop/src/update-public.xml'))
$verify=[IO.File]::ReadAllText((Join-Path $PSScriptRoot 'desktop/src/update-verify.ps1'))
$install=[IO.File]::ReadAllText((Join-Path $PSScriptRoot 'desktop/src/update-install.ps1'))
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($verify+[Environment]::NewLine+$install))
$ps=Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe'
$target=Join-Path $root 'QuickClip321.exe';Copy-Item $NightlyExe $target
$fixture=Join-Path $root 'fixture';New-Item -ItemType Directory "$fixture/state" -Force|Out-Null
$state=Join-Path $fixture 'state/state.json'
$data=@{settings=@{destination=(Join-Path $fixture 'media');yt_dlp=(Join-Path $Tools 'yt-dlp.exe');ffmpeg=(Join-Path $Tools 'ffmpeg.exe');prefs=@{language='tr';theme='Forest';run_on_startup=$false;update_channel='stable';update_mode='manual';rules=@(@{contains='fixture';category='games'});limit_kib_per_second=256}};jobs=@()}
function SaveState($d){[IO.File]::WriteAllText($state,($d|ConvertTo-Json -Depth 12 -Compress),[Text.UTF8Encoding]::new($false))}
function Assert($ok,$message){if(!$ok){throw $message}}
function StopFixture {Get-Process -ErrorAction SilentlyContinue|Where-Object {$_.Path -eq $target}|ForEach-Object{Stop-Process -Id $_.Id;try{[void]$_.WaitForExit(10000)}catch{}}}
function Swap($artifact,$version,$channel){
  $previous=(Get-FileHash $target).Hash
  $d=Get-Content $state -Raw|ConvertFrom-Json;$d.settings.prefs.update_channel=$channel;SaveState $d
  $tx=Join-Path $root ('.quickclip-update-'+[guid]::NewGuid().ToString('N'));New-Item -ItemType Directory $tx|Out-Null
  $payload=Join-Path $tx "QuickClip321-$version-win-x64.exe";Copy-Item $artifact $payload
  & (Join-Path $PSScriptRoot 'sign-release.ps1') -Artifact $payload -Version $version -Channel $channel -OutputDir $tx
  $env:QC_UPD_PUBLIC=$public
  . ([ScriptBlock]::Create($verify))
  $m=Assert-UpdateManifest $tx $version $channel
  Assert ((Assert-UpdateArtifact $tx $m) -eq $payload) 'Signature or artifact invalid'
  $opposite=if($channel -eq 'stable'){'nightly'}else{'stable'}
  $rejected=$false;try{Assert-UpdateManifest $tx $version $opposite|Out-Null}catch{$rejected=$true}
  Assert $rejected 'Wrong-channel signature accepted'
  $old=Start-Process -FilePath $target -PassThru;Start-Sleep -Seconds 2
  Assert (!$old.HasExited) 'Old fixture did not stay alive'
  $env:QC_UPD_TX=$tx;$env:QC_UPD_TARGET=$target;$env:QC_UPD_STATE=$state;$env:QC_UPD_PID=[string]$old.Id
  $env:QC_UPD_TOKEN=[IO.Path]::GetFileName($tx);$env:QC_UPD_VERSION=$version;$env:QC_UPD_CHANNEL=$channel
  $p=Start-Process -FilePath $ps -ArgumentList @('-NoLogo','-NoProfile','-NonInteractive','-EncodedCommand',$encoded) -WindowStyle Hidden -PassThru
  try{
    Start-Sleep -Milliseconds 500;Stop-Process -Id $old.Id;[void]$old.WaitForExit(10000)
    if(!$p.WaitForExit(65000)){Stop-Process -Id $p.Id;throw 'Swap timed out'}
    Assert ($p.ExitCode -eq 0) (Get-Content (Join-Path $tx 'result.txt') -Raw)
    Assert ((Get-FileHash $target).Hash -eq (Get-FileHash $artifact).Hash) 'Wrong EXE installed'
    Assert ((Get-FileHash (Join-Path $tx 'previous.exe')).Hash -eq $previous) 'Previous version not retained'
    Assert (Test-Path (Join-Path $tx 'healthy.txt')) 'No healthy UI marker'
    $s=Get-Content $state -Raw|ConvertFrom-Json
    Assert ($s.settings.prefs.update_channel -eq $channel -and $s.settings.prefs.language -eq 'tr' -and $s.settings.prefs.theme -eq 'Forest' -and $s.settings.prefs.limit_kib_per_second -eq 256 -and $s.settings.prefs.rules[0].contains -eq 'fixture') 'Settings changed during swap'
    Write-Output "PASS installed $version ($channel), healthy UI, previous EXE and settings preserved"
  }finally{StopFixture}
}
try{
  SaveState $data
  $env:QC_V3_BENCH_DIR=$fixture
  Remove-Item Env:QC_V3_SMOKE_DIR,Env:QC_V3_VISUAL,Env:QC_NOTIFICATION_FIXTURE,Env:QC_SHORTCUT_DOWNLOAD_FIXTURE -ErrorAction SilentlyContinue
  Swap $StableExe $StableVersion stable
  Swap $NightlyExe $NightlyVersion nightly
  Write-Output "PASS signed round-trip; evidence $root"
}finally{
  StopFixture
  Remove-Item Env:QC_V3_BENCH_DIR,Env:QC_UPD_TX,Env:QC_UPD_TARGET,Env:QC_UPD_STATE,Env:QC_UPD_PUBLIC,Env:QC_UPD_PID,Env:QC_UPD_TOKEN,Env:QC_UPD_VERSION,Env:QC_UPD_CHANNEL -ErrorAction SilentlyContinue
}
