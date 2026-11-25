param([string]$PythonExe = ".\.venv\Scripts\python.exe", [switch]$PublicRelease)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$venvPython = (Resolve-Path -LiteralPath $PythonExe).Path
$null = New-Item -ItemType Directory -Force -Path (Join-Path $PSScriptRoot "test-results")
$testTemp = Join-Path $PSScriptRoot ("test-results\portable-" + [guid]::NewGuid().ToString("N"))
& $venvPython -m pytest -q -p no:cacheprovider --basetemp $testTemp
if ($LASTEXITCODE -ne 0) { throw "Tests failed; build stopped" }
& $venvPython -m ruff check quickclip321 tests tools
if ($LASTEXITCODE -ne 0) { throw "Lint failed; build stopped" }
& $venvPython -m compileall -q quickclip321 tools test_launcher.py
if ($LASTEXITCODE -ne 0) { throw "Compilation failed; build stopped" }
& $venvPython tools\collect_licenses.py resources\python-notices
if ($LASTEXITCODE -ne 0) { throw "License collection failed" }
$previousPublicBuild = $env:QUICKCLIP321_PUBLIC_BUILD
try {
    $env:QUICKCLIP321_PUBLIC_BUILD = if ($PublicRelease) { "1" } else { "0" }
    & $venvPython -m PyInstaller --clean --noconfirm QuickClip321-portable.spec
    if ($LASTEXITCODE -ne 0) { throw "PyInstaller failed" }
} finally {
    $env:QUICKCLIP321_PUBLIC_BUILD = $previousPublicBuild
}
$name = if ($PublicRelease) { "QuickClip321-2.1.0-rc.1-win-x64.exe" } else { "QuickClip321-2.1.0-rc.1.exe" }
Write-Host "EXE: $PSScriptRoot\dist\$name"
Write-Host "No installation or deployment performed."
