# Fetches the bundled llama.cpp inference engine (CPU build) into
# src-tauri/resources/engine/. Run once after cloning, before `npm run tauri dev/build`.
# The GGUF models are NOT fetched here — the app downloads them on first launch.

$ErrorActionPreference = 'Stop'
$ver  = 'b9490'
$url  = "https://github.com/ggml-org/llama.cpp/releases/download/$ver/llama-$ver-bin-win-cpu-x64.zip"
$root = Split-Path -Parent $PSScriptRoot               # = app/src-tauri's parent? -> app
$dest = Join-Path $root 'src-tauri\resources\engine'
$tmp  = Join-Path $env:TEMP "llama-$ver.zip"
$ex   = Join-Path $env:TEMP "llama-$ver"

Write-Host "Downloading llama.cpp $ver (CPU) ..."
& curl.exe -L --retry 5 -o $tmp $url
Write-Host "Extracting ..."
Remove-Item -Recurse -Force $ex -ErrorAction SilentlyContinue
Expand-Archive -Path $tmp -DestinationPath $ex -Force

$srv = Get-ChildItem -Path $ex -Recurse -Filter 'llama-server.exe' | Select-Object -First 1
if (-not $srv) { throw 'llama-server.exe not found in the archive' }

New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item -Path (Join-Path $srv.DirectoryName '*') -Destination $dest -Recurse -Force
Write-Host "Engine installed to $dest"
