
$ErrorActionPreference='Stop'
function Assert-UpdateManifest($dir,$version,$channel){
 $manifest=Join-Path $dir 'update-manifest.json'
 $signature=Join-Path $dir 'update-manifest.sig'
 if((Get-Item -LiteralPath $manifest).Length -gt 16384 -or (Get-Item -LiteralPath $signature).Length -gt 2048){throw 'Oversized signed manifest'}
 $bytes=[IO.File]::ReadAllBytes($manifest)
 $sig=[Convert]::FromBase64String([IO.File]::ReadAllText($signature).Trim())
 $rsa=New-Object Security.Cryptography.RSACryptoServiceProvider(3072,(New-Object Security.Cryptography.CspParameters(24)))
 $rsa.PersistKeyInCsp=$false
 try{
  $rsa.FromXmlString($env:QC_UPD_PUBLIC)
  if(!$rsa.VerifyData($bytes,[Security.Cryptography.CryptoConfig]::MapNameToOID('SHA256'),$sig)){throw 'Invalid release signature'}
 }finally{$rsa.Dispose()}
 $m=[Text.Encoding]::UTF8.GetString($bytes)|ConvertFrom-Json
 $file="QuickClip321-$version-win-x64.exe"
 if($m.schema -ne 1 -or $m.version -cne $version -or $m.channel -cne $channel -or $m.file -cne $file -or $m.sha256 -cnotmatch '^[a-f0-9]{64}$' -or $m.size -lt 1 -or $m.size -gt 100000000){throw 'Manifest identity or limits mismatch'}
 return $m
}
function Assert-UpdateArtifact($dir,$m){
 $path=Join-Path $dir $m.file
 $item=Get-Item -LiteralPath $path
 if($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or $item.Length -ne $m.size -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $m.sha256){throw 'Release artifact integrity failure'}
 return $path
}
