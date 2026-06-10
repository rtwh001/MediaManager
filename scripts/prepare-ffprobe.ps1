$ErrorActionPreference = "Stop"

$version = "N-124881-g6028720d70-20260609"
$expectedHash = "7088D6433873B357021E3469B60214D21872ACB605CB183F34D6C536A796C3DF"
$url = "https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/assets/442872826"
$root = Split-Path -Parent $PSScriptRoot
$destination = Join-Path $root "src-tauri\binaries\ffprobe.exe"
$licenseDestination = Join-Path $root "src-tauri\third-party\ffmpeg\LICENSE.txt"
$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) "mediamanager-ffprobe"
$archive = Join-Path $temporaryRoot "ffmpeg-win64-lgpl.zip"
$extract = Join-Path $temporaryRoot "extract"

New-Item -ItemType Directory -Force -Path (Split-Path $destination), (Split-Path $licenseDestination), $temporaryRoot | Out-Null

if (Test-Path $destination) {
    $currentHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
    if ($currentHash -eq $expectedHash) {
        Write-Host "ffprobe $version is already prepared."
        exit 0
    }
}

Write-Host "Downloading the verified LGPL ffprobe build..."
curl.exe -L --retry 5 --retry-delay 3 `
    -H "Accept: application/octet-stream" `
    -H "User-Agent: MediaManager-build" `
    -o $archive $url

if (Test-Path $extract) {
    Remove-Item -LiteralPath $extract -Recurse -Force
}
Expand-Archive -LiteralPath $archive -DestinationPath $extract

$ffprobe = Get-ChildItem -LiteralPath $extract -Filter ffprobe.exe -Recurse |
    Select-Object -First 1
$license = Get-ChildItem -LiteralPath $extract -Filter LICENSE.txt -Recurse |
    Select-Object -First 1

if (-not $ffprobe -or -not $license) {
    throw "The downloaded archive does not contain the expected ffprobe files."
}

$actualHash = (Get-FileHash -LiteralPath $ffprobe.FullName -Algorithm SHA256).Hash
if ($actualHash -ne $expectedHash) {
    throw "ffprobe SHA-256 mismatch. Expected $expectedHash, received $actualHash."
}

Copy-Item -LiteralPath $ffprobe.FullName -Destination $destination -Force
Copy-Item -LiteralPath $license.FullName -Destination $licenseDestination -Force
Write-Host "Prepared ffprobe $version at $destination"
