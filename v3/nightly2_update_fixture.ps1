
param([string]$OldExe,[string]$NewExe,[string]$Tools)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot
$test=Join-Path $root ('smoke-beta3-'+[guid]::NewGuid());New-Item -ItemType Directory $test|Out-Null
$version='3.0.0-beta.6.nightly.2'
$public=[IO.File]::ReadAllText((Join-Path $PSScriptRoot 'desktop/src/update-public.xml'))
$verify=[IO.File]::ReadAllText((Join-Path $PSScriptRoot 'desktop/src/update-verify.ps1'))
$install=[IO.File]::ReadAllText((Join-Path $PSScriptRoot 'desktop/src/update-install.ps1'))
$ps=Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe'
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($verify+[Environment]::NewLine+$install))
$oldHash=(Get-FileHash $OldExe).Hash;$newHash=(Get-FileHash $NewExe).Hash
function Assert($ok,$message){if(!$ok){throw $message}}
function StopFixture($path){Get-Process -ErrorAction SilentlyContinue|Where-Object {$_.Path -eq $path}|ForEach-Object{Stop-Process -Id $_.Id;try{[void]$_.WaitForExit(10000)}catch{}}}
function NewCase($name){
 $d=Join-Path $test $name;New-Item -ItemType Directory $d|Out-Null
 $tx=Join-Path $d ('.quickclip-update-'+[guid]::NewGuid().ToString('N'));New-Item -ItemType Directory $tx|Out-Null
 $target=Join-Path $d 'QuickClip321.exe';Copy-Item $OldExe $target
 $fixture=Join-Path $d 'fixture';New-Item -ItemType Directory "$fixture/state" -Force|Out-Null
 $state=Join-Path $fixture 'state/state.json'
 $data=@{settings=@{destination=(Join-Path $fixture 'media');yt_dlp=(Join-Path $Tools 'yt-dlp.exe');ffmpeg=(Join-Path $Tools 'ffmpeg.exe');prefs=@{language='tr';theme='Forest';run_on_startup=$false;update_channel='nightly';update_mode='manual';rules=@(@{contains='fixture';category='games'})}};jobs=@()}
 [IO.File]::WriteAllText($state,($data|ConvertTo-Json -Depth 8 -Compress),[Text.UTF8Encoding]::new($false))
 return @{tx=$tx;target=$target;fixture=$fixture;state=$state}
}
function RunHelper($c,$key,$v,$oldProcess){
 $env:QC_V3_BENCH_DIR=$c.fixture
 Remove-Item Env:QC_V3_SMOKE_DIR,Env:QC_V3_VISUAL,Env:QC_NOTIFICATION_FIXTURE -ErrorAction SilentlyContinue
 $env:QC_UPD_TX=$c.tx;$env:QC_UPD_TARGET=$c.target;$env:QC_UPD_STATE=$c.state;$env:QC_UPD_PUBLIC=$key
 $env:QC_UPD_PID=if($oldProcess){[string]$oldProcess.Id}else{'2147483647'}
 $env:QC_UPD_TOKEN=[IO.Path]::GetFileName($c.tx);$env:QC_UPD_VERSION=$v;$env:QC_UPD_CHANNEL='nightly'
 $p=Start-Process -FilePath $ps -ArgumentList @('-NoLogo','-NoProfile','-NonInteractive','-EncodedCommand',$encoded) -WindowStyle Hidden -PassThru
 if($oldProcess){Start-Sleep -Milliseconds 700;Stop-Process -Id $oldProcess.Id;[void]$oldProcess.WaitForExit(10000)}
 if(!$p.WaitForExit(65000)){Stop-Process -Id $p.Id;throw 'Helper timed out'}
 Write-Host (Get-Content (Join-Path $c.tx 'result.txt') -Raw)
 return $p.ExitCode
}
$c=NewCase 'verified';$goodTx=$c.tx
$artifact=Join-Path $c.tx "QuickClip321-$version-win-x64.exe";Copy-Item $NewExe $artifact
& (Join-Path $PSScriptRoot 'sign-release.ps1') -Artifact $artifact -Version $version -Channel nightly -OutputDir $c.tx
$env:QC_UPD_PUBLIC=$public;. ([ScriptBlock]::Create($verify))
$m=Assert-UpdateManifest $c.tx $version nightly
Assert ((Assert-UpdateArtifact $c.tx $m) -eq $artifact) 'Signature failed'
$ref=[IO.File]::ReadAllBytes((Join-Path $c.tx 'update-manifest.json'))
try{[IO.File]::AppendAllText((Join-Path $c.tx 'update-manifest.json'),' ');$rejected=$false;try{Assert-UpdateManifest $c.tx $version nightly|Out-Null}catch{$rejected=$true};Assert $rejected 'Changed manifest accepted'}finally{[IO.File]::WriteAllBytes((Join-Path $c.tx 'update-manifest.json'),$ref)}
$rejected=$false;try{Assert-UpdateManifest $c.tx $version stable|Out-Null}catch{$rejected=$true};Assert $rejected 'Wrong channel accepted'
Write-Output 'PASS valid signature, altered manifest and wrong channel rejected'
$env:QC_V3_BENCH_DIR=$c.fixture;$old=Start-Process -FilePath $c.target -PassThru;Start-Sleep -Seconds 2
try{
 $r=RunHelper $c $public $version $old
 Assert ($r -eq 0) 'Signed replacement failed'
 Assert ((Get-FileHash $c.target).Hash -eq $newHash) 'Replacement mismatch'
 Assert ((Get-FileHash (Join-Path $c.tx 'previous.exe')).Hash -eq $oldHash) 'Backup missing'
 Assert (Test-Path (Join-Path $c.tx 'healthy.txt')) 'Missing healthy UI'
 $s=Get-Content $c.state -Raw|ConvertFrom-Json
 Assert ($s.settings.prefs.language -eq 'tr' -and $s.settings.prefs.theme -eq 'Forest' -and $s.settings.prefs.rules[0].contains -eq 'fixture') 'Settings lost'
 Write-Output 'PASS actual portable swap, restart health, prior EXE and settings'
}finally{StopFixture $c.target}
$c=NewCase 'tampered'
foreach($f in @('update-manifest.json','update-manifest.sig')){Copy-Item (Join-Path $goodTx $f) $c.tx}
$artifact=Join-Path $c.tx "QuickClip321-$version-win-x64.exe";Copy-Item $NewExe $artifact
$f=[IO.File]::Open($artifact,[IO.FileMode]::Open,[IO.FileAccess]::ReadWrite);try{$f.Position=128;$b=$f.ReadByte();$f.Position=128;$f.WriteByte([byte]($b -bxor 1))}finally{$f.Dispose()}
try{$r=RunHelper $c $public $version $null;Assert ($r -eq 1 -and (Get-FileHash $c.target).Hash -eq $oldHash) 'Tampered payload accepted';Write-Output 'PASS altered EXE refused before swap'}finally{StopFixture $c.target}
$c=NewCase 'rollback';$v='3.0.0-nightly.999';$name="QuickClip321-$v-win-x64.exe"
[IO.File]::WriteAllText((Join-Path $c.tx $name),'invalid fixture executable')
$m=[ordered]@{schema=1;version=$v;channel='nightly';file=$name;size=(Get-Item (Join-Path $c.tx $name)).Length;sha256=(Get-FileHash (Join-Path $c.tx $name)).Hash.ToLowerInvariant()}
$bytes=[Text.UTF8Encoding]::new($false).GetBytes(($m|ConvertTo-Json -Compress))
$rsa=New-Object Security.Cryptography.RSACryptoServiceProvider(3072,(New-Object Security.Cryptography.CspParameters(24)));$rsa.PersistKeyInCsp=$false
try{[IO.File]::WriteAllBytes((Join-Path $c.tx 'update-manifest.json'),$bytes);[IO.File]::WriteAllText((Join-Path $c.tx 'update-manifest.sig'),[Convert]::ToBase64String($rsa.SignData($bytes,[Security.Cryptography.CryptoConfig]::MapNameToOID('SHA256'))));$key=$rsa.ToXmlString($false)}finally{$rsa.Dispose()}
try{
 $r=RunHelper $c $key $v $null
 Assert ($r -eq 1 -and (Get-FileHash $c.target).Hash -eq $oldHash) 'Rollback mismatch'
 Assert ((Get-Content (Join-Path $c.tx 'result.txt') -Raw).StartsWith('Rolled back:')) 'Rollback not confirmed'
 Write-Output 'PASS signed but invalid fixture executable triggers rollback'
}finally{StopFixture $c.target}
Remove-Item Env:QC_V3_BENCH_DIR,Env:QC_UPD_TX,Env:QC_UPD_TARGET,Env:QC_UPD_STATE,Env:QC_UPD_PUBLIC,Env:QC_UPD_PID,Env:QC_UPD_TOKEN,Env:QC_UPD_VERSION,Env:QC_UPD_CHANNEL -ErrorAction SilentlyContinue
Write-Output "Update fixture evidence: $test"
