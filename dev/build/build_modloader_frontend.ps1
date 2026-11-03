param([switch]$UseInstalledDependencies, [switch]$SkipUiBuild)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$output = Join-Path $root 'build/x64'
New-Item -ItemType Directory -Force -Path $output | Out-Null
$modloaderUiSource = Join-Path $root 'modules\modloader\ui'
$modloaderUiOutput = Join-Path $root 'build\ui\modloader'
$updaterUiOutput = Join-Path $root 'build\ui\updater'
if ($SkipUiBuild) {
    if (-not (Test-Path -LiteralPath (Join-Path $modloaderUiOutput 'index.html')) -or
        -not (Test-Path -LiteralPath (Join-Path $updaterUiOutput 'updater.html'))) {
        throw 'A required frontend page is missing; remove -SkipUiBuild.'
    }
} else {
    Push-Location $modloaderUiSource
    try {
        if ($UseInstalledDependencies) {
            if (-not (Test-Path -LiteralPath (Join-Path $modloaderUiSource 'node_modules\.bin\vite.cmd'))) {
                throw 'Installed Modloader UI dependencies are missing; remove -UseInstalledDependencies.'
            }
        } elseif (Test-Path -LiteralPath (Join-Path $modloaderUiSource 'package-lock.json')) {
            & npm ci
        } else {
            & npm install
        }
        if (-not $UseInstalledDependencies -and $LASTEXITCODE -ne 0) { throw 'ShroudForge Modloader UI dependencies failed to install.' }

        & npm run build
        if ($LASTEXITCODE -ne 0) { throw 'ShroudForge Modloader UI frontend build failed.' }
    } finally {
        Pop-Location
    }
}
