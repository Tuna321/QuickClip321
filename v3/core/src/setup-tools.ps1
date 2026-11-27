# Project-owned setup. No admin, no PATH changes, no unverified extraction.
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
if (-not [Environment]::Is64BitOperatingSystem -or $env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {throw 'Managed setup requires Windows x64.'}
$root=$env:QC_MANAGED_ROOT
if ([string]::IsNullOrWhiteSpace($root) -or -not [IO.Path]::IsPathRooted($root)) {throw 'Missing application-owned tool directory.'}
$root=[IO.Path]::GetFullPath($root)
$null=New-Item -ItemType Directory -Force -Path $root
if ((Get-Item -LiteralPath $root).Attributes -band [IO.FileAttributes]::ReparsePoint) {throw 'Unsafe tool directory.'}
$target=Join-Path $root 'managed-2026.11.23-9.1-24.22.3'
function AssertHash($file,$hash){
    if (!(Test-Path -LiteralPath $file -PathType Leaf) -or ((Get-Item -LiteralPath $file).Attributes -band [IO.FileAttributes]::ReparsePoint) -or (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hash){throw 'Tool integrity verification failed.'}
}
# Report only fixed stage labels and byte counts, never URLs or local paths.
function SetupProgress($stage,$received=0L,$total=0L){
    [Console]::WriteLine("QC_SETUP|$stage|$received|$total")
    [Console]::Out.Flush()
}
function Fetch($url,$file,$hash,$max,$size=0){
    $label=[IO.Path]::GetFileName($file)
    SetupProgress "Connecting: $label"
    # Windows PowerShell uses the system proxy and TLS configuration, unlike some curl builds.
    $uri=[Uri]$url
    for($redirect=0;$redirect -le 5;$redirect++){
        if ($uri.Scheme -ne 'https'){throw 'Non-HTTPS redirect rejected.'}
        $request=[Net.HttpWebRequest]::Create($uri)
        $request.AllowAutoRedirect=$false;$request.Timeout=20000;$request.ReadWriteTimeout=20000
        $request.UserAgent='QuickClip321'
        $response=$request.GetResponse()
        try{
            $code=[int]$response.StatusCode
            if($code -ge 300 -and $code -lt 400){
                $location=$response.Headers['Location'];if(!$location){throw 'Invalid redirect.'}
                $uri=[Uri]::new($uri,$location);continue
            }
            if($code -ne 200 -or $response.ContentLength -gt $max){throw 'Invalid download response.'}
            $input=$response.GetResponseStream()
            try{
                $output=[IO.File]::Open($file,[IO.FileMode]::CreateNew)
                try{
                    $buffer=New-Object byte[] 262144;$received=0L;$clock=[Diagnostics.Stopwatch]::StartNew();$last=-1L
                    $total=[Math]::Max(0L,[long]$response.ContentLength)
                    SetupProgress "Downloading: $label" 0 $total
                    while(($n=$input.Read($buffer,0,$buffer.Length)) -gt 0){
                        $received+=$n;if($received -gt $max -or $clock.Elapsed.TotalSeconds -gt 900){throw 'Download exceeded its limit.'}
                        $output.Write($buffer,0,$n)
                        if($clock.ElapsedMilliseconds-$last -ge 250){SetupProgress "Downloading: $label" $received $total;$last=$clock.ElapsedMilliseconds}
                    }
                }finally{$output.Dispose()}
            }finally{$input.Dispose()}
            break
        }finally{$response.Dispose()}
    }
    if ($size -gt 0 -and (Get-Item -LiteralPath $file).Length -ne $size){throw 'Unexpected tool download size.'}
    SetupProgress "Verifying: $label"
    AssertHash $file $hash
}
function ExtractMember($zip,$member,$output){
    SetupProgress ('Extracting: '+[IO.Path]::GetFileName($output))
    $archive=[IO.Compression.ZipFile]::OpenRead($zip)
    try {
        $entry=$archive.GetEntry($member)
        if ($null -eq $entry -or $entry.Length -gt 250000000 -or $entry.Length -le 0){throw 'Invalid archive member.'}
        $src=$entry.Open()
        try{$dst=[IO.File]::Open($output,[IO.FileMode]::CreateNew);try{$src.CopyTo($dst)}finally{$dst.Dispose()}}finally{$src.Dispose()}
    }finally{$archive.Dispose()}
}
$expectedYt='622ed48023cac1daa52a3bf3a6ac38f03514de214b5f0fa069389fd0f6d17e5c'
$expected=[ordered]@{
    'yt-dlp.exe'=$expectedYt
    'ffmpeg.exe'='ed8f66cb877087a96f5bbb40a2944f0e764f2ad8870d5cb647360d912e62bc07'
    'ffprobe.exe'='df0199f606faa88e9ad192732a7c3987acfb647663534c297772e17730611a08'
    'node.exe'='86fc4785327db5f5c7558dc6156c543b433c313e813a222e088086222ab4afa6'
}
$files=@($expected.Keys)
$repair=$false
if (Test-Path -LiteralPath $target){
    $item=Get-Item -LiteralPath $target
    if (!$item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Unsafe managed tool directory.'}
    foreach($name in $files){
        SetupProgress "Checking: $name"
        $path=Join-Path $target $name
        if ((Test-Path -LiteralPath $path) -and ((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Unsafe managed tool file.'}
        try {AssertHash $path $expected[$name]}catch {$repair=$true}
    }
    if (!$repair){Write-Output 'Managed tools verified.';exit 0}
    SetupProgress 'Repairing damaged managed tools'
}
$stage=Join-Path $root ('.install-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory -Path $stage
try {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $out=Join-Path $stage 'verified'
    $null=New-Item -ItemType Directory -Path $out
    $cachedYt=Join-Path $root 'yt-dlp-2026.11.23/yt-dlp.exe'
    $reuseYt=$false
    if($repair){try{AssertHash $cachedYt $expectedYt;Copy-Item -LiteralPath $cachedYt -Destination (Join-Path $out 'yt-dlp.exe');$reuseYt=$true}catch{}}
    if(!$reuseYt){Fetch 'https://github.com/yt-dlp/yt-dlp/releases/download/2026.11.23/yt-dlp.exe' (Join-Path $out 'yt-dlp.exe') $expectedYt 23627865 23627865}
    $reuseFf=$false
    if($repair){try{foreach($name in @('ffmpeg.exe','ffprobe.exe')){AssertHash (Join-Path $target $name) $expected[$name]};foreach($name in @('ffmpeg.exe','ffprobe.exe','FFmpeg-LICENSE.txt')){Copy-Item -LiteralPath (Join-Path $target $name) -Destination (Join-Path $out $name)};$reuseFf=$true}catch{}}
    if(!$reuseFf){
    $ff=Join-Path $stage 'ffmpeg.zip'
    Fetch 'https://github.com/GyanD/codexffmpeg/releases/download/9.1/ffmpeg-9.1-essentials_build.zip' $ff 'b9bbbb3acece000cbeec994f856649354e52c773807841e78908374b24d658fd' 119467219 119467219
    foreach($name in @('ffmpeg.exe','ffprobe.exe')){ExtractMember $ff "ffmpeg-9.1-essentials_build/bin/$name" (Join-Path $out $name)}
    ExtractMember $ff 'ffmpeg-9.1-essentials_build/LICENSE' (Join-Path $out 'FFmpeg-LICENSE.txt')
    }
    $reuseNode=$false
    if($repair){try{AssertHash (Join-Path $target 'node.exe') $expected['node.exe'];foreach($name in @('node.exe','Node-LICENSE.txt')){Copy-Item -LiteralPath (Join-Path $target $name) -Destination (Join-Path $out $name)};$reuseNode=$true}catch{}}
    if(!$reuseNode){
    $node=Join-Path $stage 'node.zip'
    Fetch 'https://nodejs.org/dist/v24.22.3/node-v24.22.3-win-x64.zip' $node 'e612af99f626ab415ea225af6bce5c971feb448cb5cae250bb1c3fa50c5f7d9f' 100000000
    ExtractMember $node 'node-v24.22.3-win-x64/node.exe' (Join-Path $out 'node.exe')
    ExtractMember $node 'node-v24.22.3-win-x64/LICENSE' (Join-Path $out 'Node-LICENSE.txt')
    }
    $manifest=[ordered]@{}
    foreach($name in $files){SetupProgress "Verifying: $name";AssertHash (Join-Path $out $name) $expected[$name];$manifest[$name]=$expected[$name]}
    $manifest|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $out 'integrity.json') -Encoding ASCII
    # Only after every replacement is verified, preserve damaged files in quarantine.
    if ($repair){
        $quarantine=Join-Path $root ('.quarantine-'+[guid]::NewGuid().ToString('N'))
        [IO.Directory]::Move($target,$quarantine)
        try {[IO.Directory]::Move($out,$target)}catch{
            if (!(Test-Path -LiteralPath $target)){[IO.Directory]::Move($quarantine,$target)}
            throw
        }
    }else{[IO.Directory]::Move($out,$target)}
    SetupProgress 'Finalizing setup'
    Write-Output 'Managed tools installed and verified.'
    Get-Content -LiteralPath (Join-Path $target 'integrity.json')
}catch{
    $kind='Installation error'
    if($_.Exception -is [Net.WebException]){$kind='Network error: '+$_.Exception.Status}
    elseif($_.Exception.Message -match 'integrity|hash|size|archive member'){$kind='Integrity or archive verification failed'}
    elseif($_.Exception -is [UnauthorizedAccessException]){$kind='Access denied'}
    elseif($_.Exception.Message -match 'space|disk'){$kind='Check available disk space'}
    SetupProgress $kind
    throw
}finally{
    if (Test-Path -LiteralPath $stage){Remove-Item -LiteralPath $stage -Recurse -Force}
}
