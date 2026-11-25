param([string]$PythonExe = ".\.venv\Scripts\python.exe")
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
& $venvPython -m PyInstaller --clean --noconfirm QuickClip321-portable.spec
if ($LASTEXITCODE -ne 0) { throw "PyInstaller failed" }
Write-Host "Self-contained EXE: $PSScriptRoot\dist\QuickClip321-2.1.0-rc.1.exe"
Write-Host "No installation or deployment performed."
