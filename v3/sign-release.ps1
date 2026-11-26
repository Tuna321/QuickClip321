
param([Parameter(Mandatory=$true)][string]$Artifact,[Parameter(Mandatory=$true)][string]$Version,[Parameter(Mandatory=$true)][ValidateSet('stable','nightly')][string]$Channel,[Parameter(Mandatory=$true)][string]$OutputDir)
$ErrorActionPreference='Stop'
if($Version -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$'){throw 'Invalid version'}
if(($Channel -eq 'stable' -and $Version.Contains('-') -and $Version -notmatch '^\d+\.\d+\.\d+-beta\.\d+$') -or ($Channel -eq 'nightly' -and $Version -notmatch '(?:-|\.)nightly(?:\.|$)')){throw 'Version/channel mismatch'}
$item=Get-Item -LiteralPath $Artifact
$filename="QuickClip321-$Version-win-x64.exe"
if($item.Name -cne $filename -or $item.Length -lt 1 -or $item.Length -gt 100000000){throw 'Invalid release artifact'}
$manifest=[ordered]@{schema=1;version=$Version;channel=$Channel;file=$filename;size=$item.Length;sha256=(Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
$bytes=[Text.UTF8Encoding]::new($false).GetBytes(($manifest|ConvertTo-Json -Compress))
Add-Type -AssemblyName System.Security
$private=Join-Path $env:LOCALAPPDATA 'QuickClip321-Signing\release-key.dpapi'
$plain=[Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($private),$null,[Security.Cryptography.DataProtectionScope]::CurrentUser)
$rsa=New-Object Security.Cryptography.RSACryptoServiceProvider(3072,(New-Object Security.Cryptography.CspParameters(24)))
$rsa.PersistKeyInCsp=$false
try{
 $rsa.FromXmlString([Text.Encoding]::UTF8.GetString($plain))
 if($rsa.ToXmlString($false) -cne [IO.File]::ReadAllText((Join-Path $PSScriptRoot 'desktop/src/update-public.xml')).Trim()){throw 'Signing identity mismatch'}
 $signature=$rsa.SignData($bytes,[Security.Cryptography.CryptoConfig]::MapNameToOID('SHA256'))
 New-Item -ItemType Directory -Force $OutputDir|Out-Null
 $mf=Join-Path $OutputDir 'update-manifest.json';$sf=Join-Path $OutputDir 'update-manifest.sig'
 if((Test-Path $mf) -or (Test-Path $sf)){throw 'Existing manifest; refusing overwrite'}
 [IO.File]::WriteAllBytes($mf,$bytes);[IO.File]::WriteAllText($sf,[Convert]::ToBase64String($signature))
 Write-Output "Signed $filename. Publish only after release tests."
}finally{[Array]::Clear($plain,0,$plain.Length);$rsa.Dispose()}
