# Downloads the sing-box core binary that the app bundles.
$ErrorActionPreference = "Stop"

$ver  = if ($env:SINGBOX_VERSION) { $env:SINGBOX_VERSION } else { "1.14.0" }
$url  = "https://github.com/SagerNet/sing-box/releases/download/v$ver/sing-box-$ver-windows-amd64.zip"
$dest = Join-Path $PSScriptRoot "..\src-tauri\binaries"
$tmp  = Join-Path ([System.IO.Path]::GetTempPath()) ("sb-" + [guid]::NewGuid())

New-Item -ItemType Directory -Force -Path $dest, $tmp | Out-Null
Write-Host "downloading sing-box $ver ..."
Invoke-WebRequest -Uri $url -OutFile (Join-Path $tmp "sing-box.zip")
Expand-Archive -Path (Join-Path $tmp "sing-box.zip") -DestinationPath $tmp -Force
Copy-Item (Get-ChildItem -Path $tmp -Recurse -Filter "sing-box.exe" | Select-Object -First 1).FullName (Join-Path $dest "sing-box.exe") -Force
Remove-Item -Recurse -Force $tmp

Write-Host "-> $dest\sing-box.exe"
