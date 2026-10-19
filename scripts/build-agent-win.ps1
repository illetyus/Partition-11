$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$metadataText = & cargo metadata --format-version 1 --no-deps
if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed" }
$metadata = $metadataText | ConvertFrom-Json

& cargo build --locked --release --target x86_64-pc-windows-msvc -p partition11-agent
if ($LASTEXITCODE -ne 0) { throw "Windows read-only agent build failed" }

$source = Join-Path $metadata.target_directory "x86_64-pc-windows-msvc\release\partition11-agent.exe"
if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "Agent binary missing: $source" }

$destinationDirectory = Join-Path $projectRoot "resources\agent"
New-Item -ItemType Directory -Path $destinationDirectory -Force | Out-Null
$destination = Join-Path $destinationDirectory "partition11-agent.exe"
Copy-Item -LiteralPath $source -Destination $destination -Force

$sha256 = [System.Security.Cryptography.SHA256]::Create()
try {
    $sourceStream = [System.IO.File]::OpenRead($source)
    try { $sourceHash = [BitConverter]::ToString($sha256.ComputeHash($sourceStream)).Replace("-", "") }
    finally { $sourceStream.Dispose() }

    $destinationStream = [System.IO.File]::OpenRead($destination)
    try { $destinationHash = [BitConverter]::ToString($sha256.ComputeHash($destinationStream)).Replace("-", "") }
    finally { $destinationStream.Dispose() }
}
finally {
    $sha256.Dispose()
}
if ($sourceHash -ne $destinationHash) { throw "Copied agent SHA-256 mismatch" }
Write-Output "Built and staged read-only agent: $destination"
Write-Output "SHA-256: $destinationHash"
