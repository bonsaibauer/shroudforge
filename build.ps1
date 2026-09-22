param(
    [string]$BuildNumber = $(if ($env:SHROUDFORGE_BUILD_NUMBER) { $env:SHROUDFORGE_BUILD_NUMBER } else { 'dev' })
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$version = (Get-Content -LiteralPath (Join-Path $root 'VERSION') -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw "VERSION must use MAJOR.MINOR.PATCH: $version" }

$output = Join-Path $root 'build\x64'
New-Item -ItemType Directory -Force -Path $output | Out-Null
Remove-Item -LiteralPath (Join-Path $output 'licenses') -Recurse -Force -ErrorAction SilentlyContinue

$modloaderUiSource = Join-Path $root 'Shroudforge_Modules\modloader-ui\ui'
Push-Location $modloaderUiSource
try {
    if (Test-Path -LiteralPath (Join-Path $modloaderUiSource 'package-lock.json')) {
        & npm ci
    } else {
        & npm install
    }
    if ($LASTEXITCODE -ne 0) { throw 'ShroudForge Modloader UI dependencies failed to install.' }
    & npm run build
    if ($LASTEXITCODE -ne 0) { throw 'ShroudForge Modloader UI frontend build failed.' }
} finally {
    Pop-Location
}

& cargo test --manifest-path (Join-Path $root 'Cargo.toml') --release --workspace
if ($LASTEXITCODE -ne 0) { throw 'ShroudForge workspace tests failed.' }
& cargo build --manifest-path (Join-Path $root 'Cargo.toml') --release -p shroudforge-modloader
if ($LASTEXITCODE -ne 0) { throw 'ShroudForge build failed.' }
& cargo build --manifest-path (Join-Path $root 'Cargo.toml') --release -p shroudforge-debug-console
if ($LASTEXITCODE -ne 0) { throw 'ShroudForge Debug Console build failed.' }
& cargo build --manifest-path (Join-Path $root 'Cargo.toml') --release -p shroudforge-commands
if ($LASTEXITCODE -ne 0) { throw 'ShroudForge Commands build failed.' }
& cargo build --manifest-path (Join-Path $root 'Cargo.toml') --release -p shroudforge-modloader-ui
if ($LASTEXITCODE -ne 0) { throw 'ShroudForge Modloader UI build failed.' }
& cargo build --manifest-path (Join-Path $root 'Cargo.toml') --release -p shroudforge-updater
if ($LASTEXITCODE -ne 0) { throw 'ShroudForge Updater build failed.' }

$runtime = Join-Path $root 'target\release\shroudforge_modloader.dll'
$cli = Join-Path $root 'target\release\shroudforge.exe'
$debugConsole = Join-Path $root 'target\release\shroudforge-debug-console.exe'
$commands = Join-Path $root 'target\release\shroudforge-commands.exe'
$modloaderUi = Join-Path $root 'target\release\shroudforge-modloader-ui.exe'
$updater = Join-Path $root 'target\release\shroudforge-updater.exe'
if (-not (Test-Path -LiteralPath $runtime)) { throw "Runtime missing: $runtime" }
if (-not (Test-Path -LiteralPath $cli)) { throw "CLI missing: $cli" }
if (-not (Test-Path -LiteralPath $debugConsole)) { throw "Debug Console missing: $debugConsole" }
if (-not (Test-Path -LiteralPath $commands)) { throw "Commands module missing: $commands" }
if (-not (Test-Path -LiteralPath $modloaderUi)) { throw "Modloader UI missing: $modloaderUi" }
if (-not (Test-Path -LiteralPath $updater)) { throw "Updater missing: $updater" }
Copy-Item -LiteralPath $runtime -Destination (Join-Path $output 'shroudforge-runtime.dll') -Force
Copy-Item -LiteralPath $cli -Destination (Join-Path $output 'shroudforge.exe') -Force

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) { throw 'Visual Studio C++ build tools were not found.' }
$visualStudio = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $visualStudio) { throw 'Visual Studio C++ build tools were not found.' }
$msbuild = Join-Path $visualStudio 'MSBuild\Current\Bin\MSBuild.exe'
$bootstrap = Join-Path $root 'Shroudforge_Modloader\bootstrap\windows\ShroudForge.Bootstrap.vcxproj'
& $msbuild $bootstrap /m /t:Build /p:Configuration=Release /p:Platform=x64 /p:OutDir="$output\"
if ($LASTEXITCODE -ne 0) { throw 'ShroudForge Windows bootstrap build failed.' }

function Copy-ShroudForgeMods([string]$Destination) {
    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    $forbiddenLuaTokens = @(
        'get_by_qualified_hash',
        'get_by_impact_hash',
        'qualified_hash',
        'impact_hash',
        'RuntimePatch',
        'create_patch',
        'set_patch_enabled',
        'signature',
        'Payload'
    )
    foreach ($directory in Get-ChildItem -LiteralPath (Join-Path $root 'mods') -Directory) {
        $manifestPath = Join-Path $directory.FullName 'mod.json'
        $luaPath = Join-Path $directory.FullName 'src\mod.lua'
        if (-not (Test-Path -LiteralPath $manifestPath)) { continue }
        if (-not (Test-Path -LiteralPath $luaPath)) { throw "Lua entrypoint missing: $luaPath" }
        $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
        if (-not $manifest.id) { throw "Invalid mod manifest: $manifestPath" }
        foreach ($luaFile in Get-ChildItem -LiteralPath (Join-Path $directory.FullName 'src') -Filter '*.lua' -Recurse -File) {
            $source = Get-Content -LiteralPath $luaFile.FullName -Raw
            foreach ($token in $forbiddenLuaTokens) {
                if ($source -match [regex]::Escape($token)) {
                    throw "Forbidden token '$token' in Lua mod: $($luaFile.FullName)"
                }
            }
        }
        $target = Join-Path $Destination ([string]$manifest.id)
        Copy-Item -LiteralPath $directory.FullName -Destination $target -Recurse -Force
    }
}

$package = Join-Path $root 'build\package'
$archive = Join-Path $root "build\shroudforge-$version-$BuildNumber.zip"
$checksum = "$archive.sha256"
Remove-Item -LiteralPath $package -Recurse -Force -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath (Join-Path $root 'build') -Filter "shroudforge-$version-*" -File -ErrorAction SilentlyContinue |
    Remove-Item -Force
New-Item -ItemType Directory -Force -Path (Join-Path $package 'game') | Out-Null
Copy-Item -LiteralPath (Join-Path $output 'winmm.dll'),(Join-Path $output 'shroudforge-runtime.dll'),(Join-Path $output 'shroudforge.exe') -Destination (Join-Path $package 'game')
Copy-ShroudForgeMods (Join-Path $package 'game\mods')
$debugConsolePackage = Join-Path $package 'game\Shroudforge_Modules\debug-console'
New-Item -ItemType Directory -Force -Path $debugConsolePackage | Out-Null
Copy-Item -LiteralPath $debugConsole -Destination $debugConsolePackage
Copy-Item -LiteralPath (Join-Path $root 'Shroudforge_Modules\debug-console\module.json') -Destination $debugConsolePackage
$commandsPackage = Join-Path $package 'game\Shroudforge_Modules\commands'
New-Item -ItemType Directory -Force -Path $commandsPackage | Out-Null
Copy-Item -LiteralPath $commands -Destination $commandsPackage
Copy-Item -LiteralPath (Join-Path $root 'Shroudforge_Modules\commands\module.json') -Destination $commandsPackage
$modloaderUiPackage = Join-Path $package 'game\Shroudforge_Modules\modloader-ui'
New-Item -ItemType Directory -Force -Path $modloaderUiPackage | Out-Null
Copy-Item -LiteralPath $modloaderUi -Destination $modloaderUiPackage
Copy-Item -LiteralPath (Join-Path $root 'Shroudforge_Modules\modloader-ui\module.json') -Destination $modloaderUiPackage
$updaterPackage = Join-Path $package 'game\Shroudforge_Updater'
New-Item -ItemType Directory -Force -Path $updaterPackage | Out-Null
Copy-Item -LiteralPath $updater -Destination $updaterPackage
$versionManifest = [ordered]@{
    version = $version
    build = $BuildNumber
    builtAt = [DateTime]::UtcNow.ToString('o')
    updateKind = 'system'
    requiredAction = 'game_restart'
    managedPaths = @(
        'winmm.dll',
        'shroudforge-runtime.dll',
        'shroudforge.exe',
        'version.json',
        'Shroudforge_Modules',
        'Shroudforge_Updater',
        'mods/mod.flight',
        'mods/mod.infinite-item-split',
        'mods/mod.infinite-item-use',
        'mods/mod.no-fall-damage',
        'mods/mod.no-resource-cost',
        'mods/mod.no-stamina-loss',
        'mods/mod.unlock-blueprints'
    )
}
$versionManifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $package 'game\version.json') -Encoding utf8
Compress-Archive -Path "$package\*" -DestinationPath $archive
$archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath $checksum -Value "$archiveHash  $([IO.Path]::GetFileName($archive))" -Encoding ascii
Remove-Item -LiteralPath $package -Recurse -Force

Write-Host "ShroudForge $version-$BuildNumber built and packaged: $archive ($checksum)"
