# QA visual de nüün en Windows: captura cada skin × página con una raíz de QA aislada
# (NUUN_QA_ROOT en %TEMP%), sin tocar las preferencias del usuario ni ningún disco: la app corre en
# maqueta. Es el par de `--qa-skins` de la app de Mac (macos-app-v1 §10, windows-app-v1 §10).
# Las capturas son para revisar a mano; no reemplazan las del README (docs/development.md#capturas-del-producto).
# Uso: powershell -NoProfile -File scripts\windows\qa-skins.ps1 [-Out <carpeta>] [-Skins glass,win9x] [-Pages home,ingest] [-Compact]
param(
    [string]$Out = (Join-Path $env:TEMP 'nuun-qa\shots'),
    [string[]]$Skins = @('glass', 'classicMac', 'win9x', 'terminal', 'gothic'),
    [string[]]$Pages = @('home', 'ingest', 'settings'),
    [switch]$Compact,
    [string]$Exe = (Join-Path $PSScriptRoot '..\..\native\windows\Nuun\bin\Debug\net10.0-windows10.0.22000.0\win-x64\Nuun.exe'),
    [int]$Width = 1180, [int]$Height = 860
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path $Exe)) { throw "Build first: dotnet build native\windows\Nuun -c Debug ($Exe)" }
New-Item -ItemType Directory -Force -Path $Out | Out-Null
foreach ($skin in $Skins) {
    foreach ($page in $Pages) {
        $root = Join-Path $env:TEMP ("nuun-qa\root-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
        New-Item -ItemType Directory -Force -Path $root | Out-Null
        $env:NUUN_QA_ROOT = $root
        $arguments = @('--skin', $skin, '--page', $page)
        if ($Compact) { $arguments += '--compact' }
        $process = Start-Process -FilePath $Exe -ArgumentList $arguments -PassThru
        try {
            Start-Sleep -Milliseconds 3500
            $suffix = if ($Compact) { '-compact' } else { '' }
            $file = Join-Path $Out "$skin-$page$suffix.png"
            if ($Compact) { & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $process.Id -Out $file | Out-Null }
            else { & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $process.Id -Out $file -Width $Width -Height $Height | Out-Null }
            Write-Output $file
        }
        finally {
            if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
            Remove-Item Env:\NUUN_QA_ROOT -ErrorAction SilentlyContinue
            Remove-Item -Recurse -Force $root -ErrorAction SilentlyContinue
        }
    }
}
