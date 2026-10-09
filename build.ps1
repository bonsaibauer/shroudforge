param(
    [string]$BuildNumber = $(if ($env:SHROUDFORGE_BUILD_NUMBER) { $env:SHROUDFORGE_BUILD_NUMBER } else { 'dev' }),
    [switch]$SkipTests,
    [switch]$UseInstalledDependencies,
    [switch]$SkipUiBuild,
    [string]$CargoTargetDir = 'target'
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$cargoTargetPath = if ([IO.Path]::IsPathRooted($CargoTargetDir)) { [IO.Path]::GetFullPath($CargoTargetDir) } else { [IO.Path]::GetFullPath((Join-Path $root $CargoTargetDir)) }
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

$modloaderUiSource = Join-Path $root 'src\loader\modules\modloader-ui\ui'
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
        if (-not $UseInstalledDependencies -and $LASTEXITCODE -ne 0) { throw 'ShroudForge Modloader UI dependencies failed to install.' }
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
& $runtimeCmake -S $root -B (Join-Path $root 'build/native-runtime') -A x64
if ($LASTEXITCODE -ne 0) { throw 'KFC Runtime configuration failed.' }
& $runtimeCmake --build (Join-Path $root 'build/native-runtime') --config Release
if ($LASTEXITCODE -ne 0) { throw 'KFC Runtime build failed.' }
& cargo build --manifest-path (Join-Path $root 'src\parser\kfc-parser\Cargo.toml') --release -p dbghelp-proxy -p dinput8-proxy --target-dir $cargoTargetPath
if ($LASTEXITCODE -ne 0) { throw 'EML-compatible Windows proxy build failed.' }
if (-not $SkipTests) {
    & (Join-Path $root 'src/bootstrap/windows/tests/run.ps1')
    & cargo test --manifest-path (Join-Path $root 'Cargo.toml') --release --workspace
    if ($LASTEXITCODE -ne 0) { throw 'ShroudForge workspace tests failed.' }
}
& cargo build --manifest-path (Join-Path $root 'Cargo.toml') --release --workspace --target-dir $cargoTargetPath
if ($LASTEXITCODE -ne 0) { throw 'ShroudForge workspace build failed.' }

$runtime = Join-Path $cargoTargetPath 'release\shroudforge_modloader.dll'
$cli = Join-Path $cargoTargetPath 'release\shroudforge.exe'
$updater = Join-Path $cargoTargetPath 'release\shroudforge-updater.exe'
if (-not (Test-Path -LiteralPath $runtime)) { throw "Runtime missing: $runtime" }
if (-not (Test-Path -LiteralPath $cli)) { throw "Launcher missing: $cli" }
if (-not (Test-Path -LiteralPath $updater)) { throw "Standalone updater missing: $updater" }
Copy-Item -LiteralPath $runtime -Destination (Join-Path $output 'shroudforge-runtime.dll') -Force
Copy-Item -LiteralPath $cli -Destination (Join-Path $output 'shroudforge.exe') -Force
Copy-Item -LiteralPath $updater -Destination (Join-Path $output 'shroudforge-updater.exe') -Force
Copy-Item -LiteralPath (Join-Path $root 'build/native-runtime/bin/kfc-runtime.dll') -Destination $output -Force
Copy-Item -LiteralPath (Join-Path $cargoTargetPath 'release\dbghelp.dll'),(Join-Path $cargoTargetPath 'release\dinput8.dll') -Destination $output -Force
$bootstrap = Join-Path $root 'src\bootstrap\windows\ShroudForge.Bootstrap.vcxproj'
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
    foreach ($directory in Get-ChildItem -LiteralPath (Join-Path $root 'mods') -Force -Directory) {
        $manifestPath = Join-Path $directory.FullName 'mod.json'
        $luaPath = Join-Path $directory.FullName 'src\mod.lua'
        if (-not (Test-Path -LiteralPath $manifestPath)) { continue }
        if (-not (Test-Path -LiteralPath $luaPath)) { throw "Lua entrypoint missing: $luaPath" }
        $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
        if (-not $manifest.id) { throw "Invalid mod manifest: $manifestPath" }
        foreach ($luaFile in Get-ChildItem -LiteralPath (Join-Path $directory.FullName 'src') -Filter '*.lua' -Force -Recurse -File) {
            $source = Get-Content -LiteralPath $luaFile.FullName -Raw
            foreach ($token in $forbiddenLuaTokens) {
                if ($source -match [regex]::Escape($token)) {
                    throw "Forbidden token '$token' in Lua mod: $($luaFile.FullName)"
                }
            }
        }
        $target = Join-Path $Destination $directory.Name
        foreach ($file in Get-ChildItem -LiteralPath $directory.FullName -Force -Recurse -File) {
            $relative = [IO.Path]::GetRelativePath($directory.FullName, $file.FullName)
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
Copy-Item -LiteralPath (Join-Path $output 'winmm.dll'),(Join-Path $output 'dbghelp.dll'),(Join-Path $output 'dinput8.dll') -Destination $package
$dataPackage = Join-Path $package 'shroudforge'
New-Item -ItemType Directory -Force -Path $dataPackage | Out-Null
Copy-Item -LiteralPath (Join-Path $output 'shroudforge-runtime.dll'),(Join-Path $output 'shroudforge.exe'),(Join-Path $output 'shroudforge-updater.exe'),(Join-Path $output 'kfc-runtime.dll') -Destination $dataPackage
Copy-Item -LiteralPath (Join-Path $root 'src\loader\runtime\profiles') -Destination (Join-Path $dataPackage 'runtime\profiles') -Recurse -Force
Copy-ShroudForgeMods (Join-Path $package 'mods')
# Explicit release inputs: never ship an installation's generated state, events or locks.
$configSource = Join-Path $root 'src\loader\package\src\config'
$configPackage = Join-Path $dataPackage 'config'
New-Item -ItemType Directory -Force -Path $configPackage | Out-Null
Copy-Item -LiteralPath (Join-Path $configSource 'loader.default.json') -Destination (Join-Path $configPackage 'modloader-config.json') -Force
if (-not (Test-Path -LiteralPath (Join-Path $configPackage 'modloader-config.json'))) { throw 'Release configuration was not staged.' }
$versionManifest = [ordered]@{
    version = $version
    build = $BuildNumber
    builtAt = [DateTime]::UtcNow.ToString('o')
    updateKind = 'system'
    requiredAction = 'game_restart'
    managedPaths = @(Get-ChildItem -LiteralPath $package -Force -File -Recurse |
        ForEach-Object { [IO.Path]::GetRelativePath($package, $_.FullName).Replace('\', '/') } |
        Where-Object { $_ -notin @('shroudforge/config/modloader-config.json', 'shroudforge/state.json', 'shroudforge/config/.shroudforge-write.lock') }) + @('shroudforge/version.json')
}
$versionManifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $dataPackage 'version.json') -Encoding utf8
Remove-Item -LiteralPath $archive -Force -ErrorAction SilentlyContinue
[System.IO.Compression.ZipFile]::CreateFromDirectory($package, $archive, [System.IO.Compression.CompressionLevel]::Optimal, $false)
$releaseZip = [System.IO.Compression.ZipFile]::OpenRead($archive)
try {
    $entryNames = @($releaseZip.Entries | ForEach-Object { $_.FullName })
    if ($entryNames -notcontains 'winmm.dll' -or $entryNames -notcontains 'dbghelp.dll' -or $entryNames -notcontains 'dinput8.dll' -or $entryNames -notcontains 'shroudforge/shroudforge.exe' -or $entryNames -notcontains 'shroudforge/shroudforge-updater.exe' -or $entryNames -notcontains 'shroudforge/shroudforge-runtime.dll' -or $entryNames -notcontains 'shroudforge/kfc-runtime.dll' -or $entryNames -notcontains 'shroudforge/config/modloader-config.json' -or
        $entryNames -notcontains 'shroudforge/version.json' -or $entryNames -notcontains 'shroudforge/runtime/profiles/enshrouded/client/1076226.json') {
        throw 'Release archive is missing the launcher, loader configuration, or version file.'
    }
} finally {
    $releaseZip.Dispose()
}
$archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath $checksum -Value "$archiveHash  $([IO.Path]::GetFileName($archive))" -Encoding ascii
Assert-BuildPath $package
Remove-Item -LiteralPath $package -Recurse -Force

Write-Host "ShroudForge $version-$BuildNumber built and packaged: $archive ($checksum)"
