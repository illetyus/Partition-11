# Requires Windows x64, installed Microsoft Visual C toolchain + SDK.
# Deliberately compiles independent C, not a Rust restatement of guessed sizes.
# No device handles or real disk operations are opened by this check.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) {
    throw 'Visual Studio vswhere.exe not found; cannot independently verify Windows SDK ABI.'
}
$installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ($LASTEXITCODE -ne 0 -or -not $installation) {
    throw 'No installed MSVC x64 tools; refusing to skip Windows SDK ABI verification.'
}
$vcvars = Join-Path $installation 'VC\Auxiliary\Build\vcvars64.bat'
if (-not (Test-Path -LiteralPath $vcvars)) {
    throw 'vcvars64.bat unavailable; refusing to skip ABI verification.'
}

# Use the checked-in C source as the only compilation input. Keep the
# execution and executable output in the runner TEMP directory.
$source = Join-Path $PSScriptRoot '..\tests\windows\drive_layout_abi.c'
if (-not (Test-Path -LiteralPath $source)) {
    throw 'Missing independent Windows SDK ABI oracle source.'
}
$outExe = Join-Path $env:TEMP 'partition11-drive-layout-abi.exe'
$cmdFile = Join-Path $env:TEMP 'partition11-verify-drive-layout-abi.cmd'

# Use cmd.exe only to bring MSVC's vcvars environment into the same process
# that compiles and executes the oracle. A failed compiler/test propagates.
@(
    '@echo off'
    "call `"$vcvars`" >nul"
    'if errorlevel 1 exit /b 1'
    "cl /nologo /W4 /WX /TC /std:c11 /Fe:`"$outExe`" `"$source`""
    'if errorlevel 1 exit /b 1'
    "`"$outExe`""
    'if errorlevel 1 exit /b 1'
) | Set-Content -LiteralPath $cmdFile -Encoding Ascii

try {
    & cmd.exe /D /C $cmdFile
    if ($LASTEXITCODE -ne 0) {
        throw "MSVC Windows SDK ABI verification failed with exit code $LASTEXITCODE"
    }
}
finally {
    Remove-Item -LiteralPath $cmdFile, $outExe -Force -ErrorAction SilentlyContinue
}
