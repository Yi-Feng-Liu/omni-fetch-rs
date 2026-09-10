[CmdletBinding()]
param(
    [string]$ManifestPath = (Join-Path $PSScriptRoot "..\tools-manifest.json")
)

$ErrorActionPreference = "Stop"
$manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$binaryDirectory = Join-Path $projectRoot "src-tauri\binaries"
$workDirectory = Join-Path $binaryDirectory ".prepare"
$target = $manifest.targetTriple
$binaryRoot = [IO.Path]::GetFullPath($binaryDirectory).TrimEnd([IO.Path]::DirectorySeparatorChar)
$workRoot = [IO.Path]::GetFullPath($workDirectory)
if (-not $workRoot.StartsWith("$binaryRoot$([IO.Path]::DirectorySeparatorChar)", [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to use a work directory outside src-tauri/binaries"
}

New-Item -ItemType Directory -Force -Path $binaryDirectory | Out-Null
New-Item -ItemType Directory -Force -Path $workDirectory | Out-Null

function Get-VerifiedDownload {
    param(
        [Parameter(Mandatory)] [string]$Uri,
        [string]$ChecksumUri,
        [Parameter(Mandatory)] [string]$Destination,
        [string]$ChecksumFileName,
        [Parameter(Mandatory)] [string]$ExpectedChecksum
    )
    Invoke-WebRequest -Uri $Uri -OutFile $Destination -UseBasicParsing
    if ($ChecksumUri) {
        $checksumResponse = Invoke-WebRequest -Uri $ChecksumUri -UseBasicParsing
        $checksumText = if ($checksumResponse.Content -is [byte[]]) {
            [Text.Encoding]::UTF8.GetString($checksumResponse.Content).Trim()
        } else {
            ([string]$checksumResponse.Content).Trim()
        }
        if ($ChecksumFileName) {
            $line = $checksumText -split "`n" | Where-Object { $_.TrimEnd().EndsWith($ChecksumFileName) } | Select-Object -First 1
            if (-not $line) { throw "Checksum entry not found for $ChecksumFileName" }
            $published = ($line.Trim() -split "\s+")[0]
        } else {
            $published = ($checksumText -split "\s+")[0]
        }
        if ($published -ne $ExpectedChecksum) { throw "Published checksum does not match the pinned manifest value for $Uri" }
    }
    $actual = (Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash
    if ($actual -ne $ExpectedChecksum) { throw "SHA-256 mismatch for $Uri" }
}

$ytDlpDownload = Join-Path $workDirectory "yt-dlp.exe"
Get-VerifiedDownload -Uri $manifest.ytDlp.assetUrl -ChecksumUri $manifest.ytDlp.checksumUrl -Destination $ytDlpDownload -ChecksumFileName $manifest.ytDlp.checksumFileName -ExpectedChecksum $manifest.ytDlp.sha256
Copy-Item -LiteralPath $ytDlpDownload -Destination (Join-Path $binaryDirectory "yt-dlp-$target.exe") -Force
Copy-Item -LiteralPath $ytDlpDownload -Destination (Join-Path $binaryDirectory "yt-dlp.exe") -Force

$galleryDlDownload = Join-Path $workDirectory "gallery-dl.exe"
Get-VerifiedDownload -Uri $manifest.galleryDl.assetUrl -Destination $galleryDlDownload -ExpectedChecksum $manifest.galleryDl.sha256
Copy-Item -LiteralPath $galleryDlDownload -Destination (Join-Path $binaryDirectory "gallery-dl-$target.exe") -Force
Copy-Item -LiteralPath $galleryDlDownload -Destination (Join-Path $binaryDirectory "gallery-dl.exe") -Force

$ffmpegArchive = Join-Path $workDirectory "ffmpeg.zip"
$ffmpegExtract = Join-Path $workDirectory "ffmpeg"
Get-VerifiedDownload -Uri $manifest.ffmpeg.assetUrl -ChecksumUri $manifest.ffmpeg.checksumUrl -Destination $ffmpegArchive -ExpectedChecksum $manifest.ffmpeg.sha256
if (Test-Path -LiteralPath $ffmpegExtract) { Remove-Item -LiteralPath $ffmpegExtract -Recurse -Force }
Expand-Archive -LiteralPath $ffmpegArchive -DestinationPath $ffmpegExtract -Force

$ffmpeg = Get-ChildItem -LiteralPath $ffmpegExtract -Recurse -Filter "ffmpeg.exe" | Select-Object -First 1
$ffprobe = Get-ChildItem -LiteralPath $ffmpegExtract -Recurse -Filter "ffprobe.exe" | Select-Object -First 1
if (-not $ffmpeg -or -not $ffprobe) { throw "FFmpeg archive does not contain ffmpeg.exe and ffprobe.exe" }
Copy-Item -LiteralPath $ffmpeg.FullName -Destination (Join-Path $binaryDirectory "ffmpeg-$target.exe") -Force
Copy-Item -LiteralPath $ffprobe.FullName -Destination (Join-Path $binaryDirectory "ffprobe-$target.exe") -Force
Copy-Item -LiteralPath $ffmpeg.FullName -Destination (Join-Path $binaryDirectory "ffmpeg.exe") -Force
Copy-Item -LiteralPath $ffprobe.FullName -Destination (Join-Path $binaryDirectory "ffprobe.exe") -Force

Remove-Item -LiteralPath $workDirectory -Recurse -Force
Write-Host "Verified sidecars prepared in $binaryDirectory"
