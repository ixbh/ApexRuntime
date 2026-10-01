# Locate an owned Apex Legends install (Steam or EA app) and write
# tools/local-env.ps1. Read-only: it only lists folders and reads registry keys.
#   powershell -ExecutionPolicy Bypass -File tools\detect-apex.ps1

$ErrorActionPreference = 'SilentlyContinue'
$found = @()

# Steam: every library listed in libraryfolders.vdf.
$steam = (Get-ItemProperty 'HKCU:\Software\Valve\Steam').SteamPath
if ($steam) {
    $vdf = Join-Path $steam 'steamapps\libraryfolders.vdf'
    $libs = @($steam) + (Select-String -Path $vdf -Pattern '"path"\s+"([^"]+)"' |
        ForEach-Object { $_.Matches[0].Groups[1].Value -replace '\\\\', '\' })
    foreach ($lib in $libs | Select-Object -Unique) {
        $p = Join-Path $lib 'steamapps\common\Apex Legends'
        if (Test-Path (Join-Path $p 'gameversion.txt')) { $found += $p }
    }
}

# EA app / Origin: registry install dirs.
foreach ($k in 'HKLM:\SOFTWARE\Respawn\Apex', 'HKLM:\SOFTWARE\WOW6432Node\Respawn\Apex') {
    $dir = (Get-ItemProperty $k).'Install Dir'
    if ($dir -and (Test-Path $dir)) { $found += $dir.TrimEnd('\') }
}

$found = @($found | Select-Object -Unique)
if (-not $found) { Write-Host 'No Apex Legends install found.'; exit 1 }

$apex = $found[0]
$ver = Get-Content (Join-Path $apex 'gameversion.txt')
$build = Get-Content (Join-Path $apex 'build.txt')
Write-Host "Apex Legends: $apex"
Write-Host "  gameversion $ver"
Write-Host "  build       $build"

$envFile = Join-Path $PSScriptRoot 'local-env.ps1'
if (Test-Path $envFile) {
    (Get-Content $envFile) -replace "^\`$env:APEX_INSTALL\s*=.*", "`$env:APEX_INSTALL      = '$apex'   # $build" |
        Set-Content -Encoding utf8 $envFile
} else {
    (Get-Content (Join-Path $PSScriptRoot 'local-env.example.ps1')) -replace "^\`$env:APEX_INSTALL\s*=.*", "`$env:APEX_INSTALL      = '$apex'   # $build" |
        Set-Content -Encoding utf8 $envFile
}
Write-Host "Wrote $envFile"
