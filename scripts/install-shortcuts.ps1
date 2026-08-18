# Run from an unpacked Lot pack (same folder as lot-ui.exe).
# Start menu shortcut for the film-bay. Optional user PATH for lot.exe.
# Does not install Ollama, Comfy, Resolve, Blockout, or Wasserman apps.

param(
    [switch]$AddPath
)

$ErrorActionPreference = 'Stop'
$Pack = $PSScriptRoot
$Ui = Join-Path $Pack 'lot-ui.exe'
$Cli = Join-Path $Pack 'lot.exe'
if (-not (Test-Path $Ui)) {
    throw "lot-ui.exe missing next to this script"
}
if (-not (Test-Path $Cli)) {
    throw "lot.exe missing next to this script"
}

$Programs = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Lot'
New-Item -ItemType Directory -Path $Programs -Force | Out-Null
$Lnk = Join-Path $Programs 'Lot.lnk'
$W = New-Object -ComObject WScript.Shell
$S = $W.CreateShortcut($Lnk)
$S.TargetPath = $Ui
$S.WorkingDirectory = $Pack
$S.Description = 'Lot film-bay'
$S.Save()
Write-Host "start menu $Lnk"

if ($AddPath) {
    $parts = @($Pack)
    $Ff = Join-Path $Pack 'sidecar\ffmpeg'
    if (Test-Path $Ff) {
        $parts += $Ff
    }
    $current = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (-not $current) {
        $current = ''
    }
    foreach ($p in $parts) {
        $already = $current.Split(';') | Where-Object { $_ -ieq $p }
        if (-not $already) {
            if ($current) {
                $current = "$current;$p"
            }
            else {
                $current = $p
            }
        }
    }
    [Environment]::SetEnvironmentVariable('Path', $current, 'User')
    Write-Host "user PATH now includes the pack folder"
}
