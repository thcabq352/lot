# Portable Windows zip under dist/ (gitignored).
# Ships lot.exe + lot-ui.exe. Never Ollama, Comfy, Resolve, Blockout, or Wasserman apps.
# ffmpeg is GPL — Lot stays MIT OR Apache-2.0. Pass -Ffmpeg to fetch a sidecar at pack time only.

param(
    [switch]$Ffmpeg,
    [switch]$SkipBuild,
    [switch]$Installer,
    [string]$Destination
)

$ErrorActionPreference = 'Stop'
$Here = $PSScriptRoot
$Root = Split-Path -Parent $Here
if (-not $Destination) {
    $Destination = Join-Path $Root 'dist'
}

$Version = '0.1.0'
$Stage = Join-Path $Destination 'lot-windows'
$ZipName = "lot-$Version-windows-x64.zip"
$ZipPath = Join-Path $Destination $ZipName

if (-not $SkipBuild) {
    Push-Location $Root
    try {
        cargo build --release -p lot-cli -p lot-ui
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build --release -p lot-cli -p lot-ui failed"
        }
    }
    finally {
        Pop-Location
    }
}

if (Test-Path $Stage) {
    Remove-Item -Recurse -Force $Stage
}
New-Item -ItemType Directory -Path $Stage | Out-Null

$TargetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
$Cli = Join-Path $TargetRoot 'release\lot.exe'
$Ui = Join-Path $TargetRoot 'release\lot-ui.exe'
if (-not (Test-Path $Cli)) {
    throw "missing $Cli — build lot-cli first"
}
if (-not (Test-Path $Ui)) {
    throw "missing $Ui — build lot-ui first"
}
Copy-Item $Cli (Join-Path $Stage 'lot.exe')
Copy-Item $Ui (Join-Path $Stage 'lot-ui.exe')
Copy-Item (Join-Path $Here 'install-shortcuts.ps1') (Join-Path $Stage 'install-shortcuts.ps1')
Copy-Item (Join-Path $Here 'lot.nsi') (Join-Path $Stage 'lot.nsi')

$Readme = @'
Lot — Windows portable pack
===========================

This folder is the filmmaker wrap. Unzip and run. Cargo is not required.

Included
  lot.exe      CLI + lot mcp
  lot-ui.exe   film-bay window
  install-shortcuts.ps1
    Start menu shortcut for the film-bay.
    Optional user PATH:  .\install-shortcuts.ps1 -AddPath
  lot.nsi      NSIS script. pack-windows.ps1 -Installer builds setup.exe when makensis is on PATH.

Not included (install yourself only if you want them)
  Ollama          local brain
  ComfyUI         local stills
  DaVinci Resolve optional FCPXML/EDL interchange
  Blockout / Motion Previs Studio   optional other programs
  ScriptBreak, Cork Board, Master Canvas, Slate, Circle Take, or any Wasserman app

ffmpeg (GPL sidecar)
  Lot is MIT OR Apache-2.0. ffmpeg is GPL. It is never committed to the Lot repo.
  Default pack does not bundle it. lot doctor then reports: no ffmpeg —
  Finish / plate-look need ffmpeg on PATH, or sidecar\ffmpeg\ next to lot.exe.
  To fetch a sidecar at pack time (not into git):

    .\scripts\pack-windows.ps1 -Ffmpeg

  Override the zip with LOT_FFMPEG_ZIP if you already downloaded a build.
  Source used by the pack script: https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip

First call
  .\lot.exe status --json
  .\lot.exe doctor --json
  .\lot-ui.exe

Agents:  .\lot.exe mcp
'@
Set-Content -Path (Join-Path $Stage 'README.txt') -Value $Readme -Encoding utf8

if ($Ffmpeg) {
    $Cache = Join-Path $Destination 'cache'
    New-Item -ItemType Directory -Path $Cache -Force | Out-Null
    $FfZip = Join-Path $Cache 'ffmpeg-release-essentials.zip'
    if ($env:LOT_FFMPEG_ZIP -and (Test-Path $env:LOT_FFMPEG_ZIP)) {
        $FfZip = $env:LOT_FFMPEG_ZIP
    }
    elseif (-not (Test-Path $FfZip)) {
        $Url = 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip'
        Write-Host "fetching GPL ffmpeg sidecar from $Url"
        Invoke-WebRequest -Uri $Url -OutFile $FfZip
    }
    $Extract = Join-Path $Cache 'ffmpeg-extract'
    if (Test-Path $Extract) {
        Remove-Item -Recurse -Force $Extract
    }
    Expand-Archive -Path $FfZip -DestinationPath $Extract
    $FfExe = Get-ChildItem -Path $Extract -Recurse -Filter ffmpeg.exe | Select-Object -First 1
    $FpExe = Get-ChildItem -Path $Extract -Recurse -Filter ffprobe.exe | Select-Object -First 1
    if (-not $FfExe -or -not $FpExe) {
        throw "ffmpeg zip had no ffmpeg.exe / ffprobe.exe"
    }
    $Sidecar = Join-Path $Stage 'sidecar\ffmpeg'
    New-Item -ItemType Directory -Path $Sidecar -Force | Out-Null
    Copy-Item $FfExe.FullName (Join-Path $Sidecar 'ffmpeg.exe')
    Copy-Item $FpExe.FullName (Join-Path $Sidecar 'ffprobe.exe')
    $License = Get-ChildItem -Path $Extract -Recurse -Include 'LICENSE','LICENSE.txt','COPYING.GPLv3','COPYING' |
        Select-Object -First 3
    foreach ($lic in $License) {
        Copy-Item $lic.FullName (Join-Path $Sidecar $lic.Name)
    }
    $Gpl = @'
ffmpeg + ffprobe sidecar
========================

These two binaries are NOT Lot. They are the GPL ffmpeg project
(https://ffmpeg.org/), fetched at pack time from
https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip

Lot remains MIT OR Apache-2.0. This sidecar is a clearly marked
optional companion. Do not copy it into the Lot git tree.
'@
    Set-Content -Path (Join-Path $Sidecar 'README-GPL.txt') -Value $Gpl -Encoding utf8
}

if (Test-Path $ZipPath) {
    Remove-Item -Force $ZipPath
}
Compress-Archive -Path (Join-Path $Stage '*') -DestinationPath $ZipPath
Write-Host "packed $ZipPath"
Write-Host "stage  $Stage"

if ($Installer) {
    $Nsi = Join-Path $Stage 'lot.nsi'
    $Setup = Join-Path $Destination "lot-$Version-windows-x64-setup.exe"
    $makensis = Get-Command makensis -ErrorAction SilentlyContinue
    if (-not $makensis) {
        Write-Host "no makensis — zip is ready; install NSIS to build $Setup"
        return
    }
    Push-Location $Stage
    try {
        & $makensis.Source $Nsi
        if ($LASTEXITCODE -ne 0) {
            throw "makensis failed"
        }
        $built = Join-Path $Stage 'lot-setup.exe'
        if (-not (Test-Path $built)) {
            throw "makensis wrote no setup.exe"
        }
        Copy-Item $built $Setup -Force
        Write-Host "installer $Setup"
    }
    finally {
        Pop-Location
    }
}
