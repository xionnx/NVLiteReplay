$ErrorActionPreference = "Stop"

# Ensure Cargo & MinGW bin directories are in PATH for current session
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoBin) {
    $env:Path = "$cargoBin;$env:Path"
}

$winLibsDirs = Get-ChildItem -Path "$env:LOCALAPPDATA\Microsoft\WinGet\Packages" -Filter "BrechtSanders.WinLibs*" -Directory -ErrorAction SilentlyContinue
foreach ($pkg in $winLibsDirs) {
    $mingwBin = Join-Path $pkg.FullName "mingw64\bin"
    if (Test-Path $mingwBin) {
        $env:Path = "$mingwBin;$env:Path"
    }
}

Write-Host "Building NVLiteReplayRustVoiceRecog.exe (Release Portable)..."
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo build failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

$exeSrc = Join-Path $PSScriptRoot "target\release\NVLiteReplayRustVoiceRecog.exe"
$exeDst = Join-Path $PSScriptRoot "NVLiteReplayRustVoiceRecog.exe"
if (Test-Path $exeSrc) {
    Copy-Item -Path $exeSrc -Destination $exeDst -Force
    $kb = [math]::Round((Get-Item $exeDst).Length / 1KB, 1)
    Write-Host "Build SUCCEEDED: $exeDst ($kb KB)" -ForegroundColor Green
} else {
    Write-Error "Build output not found at $exeSrc"
}
