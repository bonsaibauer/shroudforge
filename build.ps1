param(
    [string]$BuildNumber = $(if ($env:SHROUDFORGE_BUILD_NUMBER) { $env:SHROUDFORGE_BUILD_NUMBER } else { 'dev' }),
    [switch]$SkipTests,
    [switch]$UseInstalledDependencies,
    [switch]$SkipUiBuild
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
if ($BuildNumber -notmatch '^[A-Za-z0-9._-]+$') { throw 'BuildNumber contains invalid path characters.' }
function Assert-BuildPath([string]$Path) {
    $resolved = [IO.Path]::GetFullPath($Path)
    $boundary = [IO.Path]::GetFullPath((Join-Path $root 'build')) + [IO.Path]::DirectorySeparatorChar
    if (-not $resolved.StartsWith($boundary, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Build cleanup target escapes build directory: $resolved"
    }
    if ((Test-Path -LiteralPath $resolved) -and ((Get-Item -LiteralPath $resolved).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Build cleanup target is a reparse point: $resolved"
    }
}
$version = (Get-Content -LiteralPath (Join-Path $root 'VERSION') -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw "VERSION must use MAJOR.MINOR.PATCH: $version" }

$output = Join-Path $root 'build\x64'
New-Item -ItemType Directory -Force -Path $output | Out-Null

$modloaderUiSource = Join-Path $root 'Shroudforge_Modules\modloader-ui\ui'
if ($SkipUiBuild) {
    if (-not (Test-Path -LiteralPath (Join-Path $modloaderUiSource 'dist\index.html'))) {
        throw 'The built Modloader UI is missing; remove -SkipUiBuild.'
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
        if ($LASTEXITCODE -ne 0) { throw 'ShroudForge Modloader UI dependencies failed to install.' }
        & npm run build
        if ($LASTEXITCODE -ne 0) { throw 'ShroudForge Modloader UI frontend build failed.' }
    } finally {
        Pop-Location
    }
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) { throw 'Visual Studio C++ build tools were not found.' }
$visualStudio = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $visualStudio) { throw 'Visual Studio C++ build tools were not found.' }
$msbuild = Join-Path $visualStudio 'MSBuild\Current\Bin\MSBuild.exe'
$runtimeCmake = Join-Path $visualStudio 'Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe'
& $runtimeCmake -S (Join-Path $root 'Shroudforge_Modloader/kfc-runtime') -B (Join-Path $root 'build/native-runtime') -A x64
if ($LASTEXITCODE -ne 0) { throw 'KFC Runtime configuration failed.' }
& $runtimeCmake --build (Join-Path $root 'build/native-runtime') --config Release
if ($LASTEXITCODE -ne 0) { throw 'KFC Runtime build failed.' }
& $runtimeCmake -S (Join-Path $root 'Shroudforge_Modules/runtime-diagnostics') -B (Join-Path $root 'build/native-diagnostics') -A x64
if ($LASTEXITCODE -ne 0) { throw 'Runtime diagnostics configuration failed.' }
& $runtimeCmake --build (Join-Path $root 'build/native-diagnostics') --config Release
if ($LASTEXITCODE -ne 0) { throw 'Runtime diagnostics build failed.' }

if (-not $SkipTests) {
    & cargo test --manifest-path (Join-Path $root 'Cargo.toml') --release --workspace
    if ($LASTEXITCODE -ne 0) { throw 'ShroudForge workspace tests failed.' }
}
& cargo build --manifest-path (Join-Path $root 'Cargo.toml') --release --workspace
if ($LASTEXITCODE -ne 0) { throw 'ShroudForge workspace build failed.' }

$runtime = Join-Path $root 'target\release\shroudforge_modloader.dll'
$cli = Join-Path $root 'target\release\shroudforge.exe'
if (-not (Test-Path -LiteralPath $runtime)) { throw "Runtime missing: $runtime" }
if (-not (Test-Path -LiteralPath $cli)) { throw "Launcher missing: $cli" }
Copy-Item -LiteralPath $runtime -Destination (Join-Path $output 'shroudforge-runtime.dll') -Force
Copy-Item -LiteralPath $cli -Destination (Join-Path $output 'shroudforge.exe') -Force
Copy-Item -LiteralPath (Join-Path $root 'build/native-runtime/Release/kfc-runtime.dll') -Destination $output -Force
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
        foreach ($file in Get-ChildItem -LiteralPath $directory.FullName -Recurse -File) {
            $relative = [IO.Path]::GetRelativePath($directory.FullName, $file.FullName)
            if ($relative -match '(^|[\\/])\.[^\\/]+([\\/]|$)' -or $file.Name -eq '.mod-json.lock') { continue }
            $destinationFile = Join-Path $target $relative
            New-Item -ItemType Directory -Force -Path (Split-Path -Parent $destinationFile) | Out-Null
            Copy-Item -LiteralPath $file.FullName -Destination $destinationFile -Force
        }
    }
}

$package = Join-Path $root 'build\package'
$archive = Join-Path $root "build\shroudforge-$version-$BuildNumber.zip"
$checksum = "$archive.sha256"
Assert-BuildPath $package
Assert-BuildPath $archive
Remove-Item -LiteralPath $package -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $package | Out-Null
Copy-Item -LiteralPath (Join-Path $output 'winmm.dll'),(Join-Path $output 'shroudforge-runtime.dll'),(Join-Path $output 'shroudforge.exe') -Destination $package
& $runtimeCmake --install (Join-Path $root 'build/native-runtime') --config Release --prefix $package
if ($LASTEXITCODE -ne 0) { throw 'KFC Runtime packaging failed.' }
Copy-ShroudForgeMods (Join-Path $package 'mods')
# Explicit release inputs: never ship an installation's generated state, events or locks.
$configSource = Join-Path $root 'config'
$configPackage = Join-Path $package 'config'
New-Item -ItemType Directory -Force -Path $configPackage | Out-Null
Copy-Item -LiteralPath (Join-Path $configSource 'shroudforge.json') -Destination $configPackage -Force
if (-not (Test-Path -LiteralPath (Join-Path $configPackage 'shroudforge.json'))) { throw 'Release configuration was not staged.' }
if ((Test-Path -LiteralPath (Join-Path $package 'Shroudforge_Modules')) -or
    (Test-Path -LiteralPath (Join-Path $package 'Shroudforge_Updater'))) {
    throw 'Standalone module directories must not be included in the release.'
}
$versionManifest = [ordered]@{
    version = $version
    build = $BuildNumber
    builtAt = [DateTime]::UtcNow.ToString('o')
    updateKind = 'system'
    requiredAction = 'game_restart'
    managedPaths = @(Get-ChildItem -LiteralPath $package -File -Recurse |
        ForEach-Object { [IO.Path]::GetRelativePath($package, $_.FullName).Replace('\', '/') } |
        Where-Object { $_ -notin @('config/shroudforge.json', 'config/state.json') }) + @('version.json')
}
$versionManifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $package 'version.json') -Encoding utf8
Compress-Archive -Path "$package\*" -DestinationPath $archive -Force
$releaseZip = [System.IO.Compression.ZipFile]::OpenRead($archive)
try {
    $entryNames = @($releaseZip.Entries | ForEach-Object { $_.FullName })
    if ($entryNames -notcontains 'shroudforge.exe' -or $entryNames -notcontains 'config/shroudforge.json' -or
        $entryNames -notcontains 'version.json') {
        throw 'Release archive is missing the launcher, loader configuration, or version file.'
    }
    if ($entryNames | Where-Object { $_ -like 'Shroudforge_Modules/*' -or $_ -like 'Shroudforge_Updater/*' }) {
        throw 'Release archive contains a standalone module directory.'
    }
} finally {
    $releaseZip.Dispose()
}
$archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath $checksum -Value "$archiveHash  $([IO.Path]::GetFileName($archive))" -Encoding ascii
Assert-BuildPath $package
Remove-Item -LiteralPath $package -Recurse -Force

Write-Host "ShroudForge $version-$BuildNumber built and packaged: $archive ($checksum)"
