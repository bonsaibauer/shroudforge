param(
    [string]$BuildNumber = $(if ($env:SHROUDFORGE_BUILD_NUMBER) { $env:SHROUDFORGE_BUILD_NUMBER } else { 'dev' }),
    [string]$Version,
    [ValidateSet('dev', 'release')]
    [string]$BuildType = 'dev',
    [switch]$WithoutUpdater
)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$output = Join-Path $root 'build/x64'
New-Item -ItemType Directory -Force -Path $output | Out-Null
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
if (-not $PSBoundParameters.ContainsKey('Version')) {
    $Version = (Get-Content -LiteralPath (Join-Path $root 'VERSION') -Raw).Trim()
}
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw "VERSION must use MAJOR.MINOR.PATCH: $version" }

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
                if ($source -cmatch [regex]::Escape($token)) {
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
$licenses = Join-Path $dataPackage 'licenses'
New-Item -ItemType Directory -Force -Path $licenses | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'modules/hotkeys/SDL/LICENSE.txt') -Destination (Join-Path $licenses 'SDL3.txt')
Copy-Item -LiteralPath (Join-Path $output 'shroudforge-runtime.dll'),(Join-Path $output 'shroudforge.exe'),(Join-Path $output 'kfc-runtime.dll') -Destination $dataPackage
if (-not $WithoutUpdater) {
    Copy-Item -LiteralPath (Join-Path $output 'shroudforge-updater.exe') -Destination $dataPackage
}
New-Item -ItemType Directory -Force -Path (Join-Path $dataPackage 'runtime\profiles') | Out-Null
Get-ChildItem -LiteralPath (Join-Path $root 'modules\game-runtime\profiles') -Exclude 'dev' | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $dataPackage 'runtime\profiles') -Recurse -Force }
Copy-Item -LiteralPath (Join-Path $root 'modules\modloader\mod-profiles') -Destination (Join-Path $dataPackage 'runtime\profiles\mod-profiles') -Recurse -Force
Copy-ShroudForgeMods (Join-Path $package 'mods')
# Explicit release inputs: never ship an installation's generated state, events or locks.
$configSource = Join-Path $root 'modules/loader-config'
$configPackage = Join-Path $dataPackage 'config'
New-Item -ItemType Directory -Force -Path $configPackage | Out-Null
Copy-Item -LiteralPath (Join-Path $configSource 'loader.default.json') -Destination (Join-Path $configPackage 'modloader-config.json') -Force
if (-not (Test-Path -LiteralPath (Join-Path $configPackage 'modloader-config.json'))) { throw 'Release configuration was not staged.' }
$versionManifest = [ordered]@{
    version = $version
    build = $BuildNumber
    buildType = $BuildType
    builtAt = [DateTime]::UtcNow.ToString('o')
    updateKind = 'system'
    updaterIncluded = -not $WithoutUpdater
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
    $updaterEntryPresent = $entryNames -contains 'shroudforge/shroudforge-updater.exe'
    if ($entryNames -notcontains 'winmm.dll' -or $entryNames -notcontains 'dbghelp.dll' -or $entryNames -notcontains 'dinput8.dll' -or $entryNames -notcontains 'shroudforge/shroudforge.exe' -or (-not $WithoutUpdater -and -not $updaterEntryPresent) -or ($WithoutUpdater -and $updaterEntryPresent) -or $entryNames -notcontains 'shroudforge/shroudforge-runtime.dll' -or $entryNames -notcontains 'shroudforge/kfc-runtime.dll' -or $entryNames -notcontains 'shroudforge/config/modloader-config.json' -or
        $entryNames -notcontains 'shroudforge/version.json' -or $entryNames -notcontains 'shroudforge/runtime/profiles/enshrouded/client/1076226.json' -or $entryNames -notcontains 'shroudforge/runtime/profiles/mod-profiles/README.md' -or $entryNames -notcontains 'shroudforge/runtime/profiles/mod-profiles/creative.json') {
        throw 'Release archive is missing the launcher, loader configuration, version file, or mod compatibility profiles.'
    }
} finally {
    $releaseZip.Dispose()
}
$archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath $checksum -Value "$archiveHash  $([IO.Path]::GetFileName($archive))" -Encoding ascii
Assert-BuildPath $package
Remove-Item -LiteralPath $package -Recurse -Force

Write-Host "ShroudForge $version-$BuildNumber built and packaged: $archive ($checksum)"
