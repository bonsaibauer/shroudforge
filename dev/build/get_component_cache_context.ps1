param(
    [Parameter(Mandatory)]
    [ValidateSet('proxies', 'sdl', 'rust')]
    [string]$Component,
    [switch]$NoExport
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))

function Get-TextHash([string]$Text) {
    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()
}

function Get-WorkingFiles([string]$Directory) {
    # -z preserves spaces, Unicode and newlines in Git paths. Include local edits
    # and untracked source files rather than trusting the submodule commit alone.
    $listing = (& git -C $Directory ls-files --cached --others --exclude-standard -z 2>$null) -join "`n"
    if ($LASTEXITCODE -ne 0) {
        # A local path dependency can live outside a Git checkout.
        $pendingDirectories = [Collections.Generic.Stack[string]]::new()
        $pendingDirectories.Push($Directory)
        while ($pendingDirectories.Count -gt 0) {
            foreach ($entry in Get-ChildItem -LiteralPath $pendingDirectories.Pop() -Force) {
                if ($entry.PSIsContainer) {
                    if ($entry.Name -notin @('.git', 'target', 'node_modules')) { $pendingDirectories.Push($entry.FullName) }
                } else { $entry.FullName }
            }
        }
        $global:LASTEXITCODE = 0
        return
    }
    foreach ($relative in $listing.Split([char]0, [StringSplitOptions]::RemoveEmptyEntries)) {
        $path = Join-Path $Directory $relative
        if (Test-Path -LiteralPath $path -PathType Container) {
            if (Test-Path -LiteralPath (Join-Path $path '.git')) { Get-WorkingFiles $path }
        } else {
            $path
        }
    }
}

function Get-FileRecords([string[]]$Paths) {
    foreach ($path in @($Paths | Sort-Object -Unique)) {
        $label = [IO.Path]::GetRelativePath($root, $path).Replace('\', '/')
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            "$label=$((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant())"
        } else {
            "$label=<missing>"
        }
    }
}

function Get-ToolRecord([string]$Name, [string[]]$VersionArguments) {
    $command = Get-Command $Name -CommandType Application -ErrorAction Stop | Select-Object -First 1
    $version = (& $command.Source @VersionArguments) -join "`n"
    if ($LASTEXITCODE -ne 0) { throw "Cannot fingerprint $Name" }
    [ordered]@{
        path = $command.Source
        sha256 = (Get-FileHash -LiteralPath $command.Source -Algorithm SHA256).Hash
        version = $version
    }
}

function Get-CargoConfigurationFiles {
    $directory = [IO.DirectoryInfo]::new($root)
    while ($null -ne $directory) {
        foreach ($name in @('config', 'config.toml')) {
            $path = Join-Path $directory.FullName ".cargo/$name"
            if (Test-Path -LiteralPath $path -PathType Leaf) { $path }
        }
        $directory = $directory.Parent
    }
    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path ([Environment]::GetFolderPath('UserProfile')) '.cargo' }
    foreach ($name in @('config', 'config.toml')) {
        $path = Join-Path $cargoHome $name
        if (Test-Path -LiteralPath $path -PathType Leaf) { $path }
    }
    foreach ($name in @('.cargo/config', '.cargo/config.toml', 'rust-toolchain', 'rust-toolchain.toml', 'kfc-parser/.cargo/config', 'kfc-parser/.cargo/config.toml', 'kfc-parser/rust-toolchain', 'kfc-parser/rust-toolchain.toml')) {
        $path = Join-Path $root $name
        if (Test-Path -LiteralPath $path -PathType Leaf) { $path }
    }
}

function Get-ProxyLocalDependencyFiles {
    $pending = [Collections.Generic.Queue[string]]::new()
    $pending.Enqueue((Join-Path $root 'kfc-parser/Cargo.toml'))
    $visited = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    while ($pending.Count -gt 0) {
        $manifestPath = $pending.Dequeue()
        if (-not $visited.Add($manifestPath)) { continue }
        $metadataText = (& cargo metadata --manifest-path $manifestPath --locked --offline --no-deps --format-version 1) -join "`n"
        if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect local proxy build dependencies.' }
        $metadata = $metadataText | ConvertFrom-Json
        foreach ($package in $metadata.packages) { [void]$visited.Add([IO.Path]::GetFullPath($package.manifest_path)) }
        foreach ($package in $metadata.packages) {
            $packageDirectory = Split-Path -Parent $package.manifest_path
            $parserDirectory = Join-Path $root 'kfc-parser'
            if (-not ($packageDirectory.Equals($parserDirectory, [StringComparison]::OrdinalIgnoreCase) -or
                $packageDirectory.StartsWith($parserDirectory + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase))) {
                Get-WorkingFiles $packageDirectory
            }
            foreach ($dependency in $package.dependencies) {
                if ($dependency.path) {
                    $dependencyManifest = Join-Path ([IO.Path]::GetFullPath($dependency.path)) 'Cargo.toml'
                    if (-not $visited.Contains($dependencyManifest)) { $pending.Enqueue($dependencyManifest) }
                }
            }
        }
    }
}

$rustc = Get-ToolRecord 'rustc' @('-Vv')
$cargo = Get-ToolRecord 'cargo' @('-V')
$visualStudio = & (Join-Path $PSScriptRoot 'find_visual_studio.ps1')
$toolsVersion = (Get-Content -LiteralPath (Join-Path $visualStudio 'VC/Auxiliary/Build/Microsoft.VCToolsVersion.default.txt') -Raw).Trim()
$toolsDirectory = if ($env:VCToolsInstallDir) { $env:VCToolsInstallDir } else { Join-Path $visualStudio "VC/Tools/MSVC/$toolsVersion" }
$msvcFiles = @('cl.exe', 'c1.dll', 'c1xx.dll', 'c2.dll', 'link.exe', 'lib.exe') | ForEach-Object { Join-Path $toolsDirectory "bin/Hostx64/x64/$_" }
if (-not (Test-Path -LiteralPath $msvcFiles[0] -PathType Leaf)) { throw 'The selected x64 MSVC compiler is missing.' }

$sdkDirectory = $env:WindowsSdkDir
if (-not $sdkDirectory) {
    $kits = Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots' -ErrorAction SilentlyContinue
    $sdkDirectory = $kits.KitsRoot10
}
if (-not $sdkDirectory) { $sdkDirectory = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10' }
$sdkVersions = @(Get-ChildItem -LiteralPath (Join-Path $sdkDirectory 'Include') -Directory | Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } | Sort-Object { [version]$_.Name })
$sdkVersion = if ($env:WindowsSDKVersion) { $env:WindowsSDKVersion.TrimEnd('\', '/') } elseif ($sdkVersions.Count -gt 0) { $sdkVersions[-1].Name } else { throw 'No Windows SDK headers were found.' }
$sdkFiles = @(
    (Join-Path $sdkDirectory 'SDKManifest.xml'),
    (Join-Path $sdkDirectory "Include/$sdkVersion/um/Windows.h"),
    (Join-Path $sdkDirectory "Include/$sdkVersion/ucrt/corecrt.h"),
    (Join-Path $sdkDirectory "Lib/$sdkVersion/um/x64/kernel32.lib"),
    (Join-Path $sdkDirectory "Lib/$sdkVersion/ucrt/x64/ucrt.lib"),
    (Join-Path $sdkDirectory "bin/$sdkVersion/x64/rc.exe"),
    (Join-Path $sdkDirectory "bin/$sdkVersion/x64/mt.exe")
)
if (-not (Test-Path -LiteralPath $sdkFiles[1] -PathType Leaf)) { throw "The selected Windows SDK headers are missing: $sdkVersion" }

$flags = [ordered]@{}
$compilerOverrides = @()
foreach ($variable in @(Get-ChildItem Env: | Sort-Object Name)) {
    if ($variable.Name -match '^(RUST|CARGO|CC($|_)|CXX($|_)|AR($|_)|CFLAGS|CXXFLAGS|CPPFLAGS|LDFLAGS|CL$|_CL_$|LINK$|_LINK_$|CMAKE|VCTools|VCINSTALLDIR|VSINSTALLDIR|VisualStudioVersion|WindowsSdk|WindowsSDK|UCRT|INCLUDE$|LIB$|LIBPATH$|SDKROOT$|TARGET$|HOST$|NUM_JOBS$|PROFILE$|OPT_LEVEL$|DEBUG$)' -and
        $variable.Name -notmatch '(TOKEN|PASSWORD|SECRET|CREDENTIAL|CARGO_TARGET_DIR)') {
        $flags[$variable.Name] = $variable.Value
        if ($variable.Name -match '^(RUSTC|RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|CARGO_BUILD_RUSTC|CC|CXX|AR|CMAKE|CARGO_TARGET_.+_LINKER)$') {
            $override = Get-Command $variable.Value -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
            if ($override) { $compilerOverrides += $override.Source }
        }
    }
}

$toolchain = [ordered]@{
    platform = [Environment]::OSVersion.Platform.ToString()
    architecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    imageOs = $env:ImageOS
    imageVersion = $env:ImageVersion
    rustc = $rustc
    cargo = $cargo
    visualStudio = $visualStudio
    msvcVersion = $toolsVersion
    msvcFiles = @(Get-FileRecords $msvcFiles)
    sdkDirectory = $sdkDirectory
    sdkVersion = $sdkVersion
    sdkVersions = @($sdkVersions.Name)
    sdkFiles = @(Get-FileRecords $sdkFiles)
    overrides = @(Get-FileRecords $compilerOverrides)
    flags = $flags
}
$configurationFiles = @(Get-CargoConfigurationFiles)
$recipeFiles = @($PSCommandPath, (Join-Path $PSScriptRoot 'find_visual_studio.ps1'))
$inputs = @()
$recipe = ''
$repositoryIndex = $null
switch ($Component) {
    'proxies' {
        # Future build scripts can consume inputs outside the parser directory.
        # Conservatively include all versioned/non-ignored repository sources.
        $inputs += @(Get-WorkingFiles $root)
        $inputs += @(Get-WorkingFiles (Join-Path $root 'kfc-parser'))
        $inputs += @(Get-ProxyLocalDependencyFiles)
        # Gitlinks remain visible even when their submodule working tree has
        # not been initialized. Do not mutate or refresh the caller's index.
        $stageListing = (& git -C $root ls-files --stage -z) -join "`n"
        if ($LASTEXITCODE -ne 0) { throw 'Cannot fingerprint repository Gitlinks.' }
        $repositoryIndex = Get-TextHash $stageListing
        $recipeFiles += Join-Path $PSScriptRoot 'build_windows_proxies.ps1'
        $recipe = 'cargo build --locked --release -p dbghelp-proxy -p dinput8-proxy, parser workspace profile and default rustc host target'
    }
    'sdl' {
        $inputs += @(Get-WorkingFiles (Join-Path $root 'modules/hotkeys/SDL'))
        $inputs += @(Get-WorkingFiles $root | Where-Object { $_.StartsWith((Join-Path $root 'modules/hotkeys') + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -and -not $_.StartsWith((Join-Path $root 'modules/hotkeys/SDL') + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) })
        $inputs += @((Join-Path $root 'Cargo.toml'), (Join-Path $root 'Cargo.lock'))
        $recipe = 'SDL static x64 library, Cargo release profile, target CRT and native build recipe in modules/hotkeys'
        $toolchain.cmake = Get-ToolRecord 'cmake' @('--version')
    }
    'rust' {
        # rust-cache adds lockfiles to its full key and can reuse unchanged
        # dependencies across lock updates. Keep profile/config contracts in its
        # stable prefix without tying that prefix to npm or packaging scripts.
        $inputs += @(Get-WorkingFiles $root | Where-Object { [IO.Path]::GetFileName($_) -eq 'Cargo.toml' })
        $recipe = 'external Cargo dependencies only, release profile, workspace crates and installed binaries excluded'
    }
}
$inputRecords = @(Get-FileRecords @($inputs + $configurationFiles + $recipeFiles))
$context = [ordered]@{
    format = 1
    component = $Component
    repositoryRoot = $root
    recipe = $recipe
    sourceRecipeHash = Get-TextHash ($inputRecords -join "`n")
    repositoryIndex = $repositoryIndex
    toolchain = $toolchain
}
$key = Get-TextHash ($context | ConvertTo-Json -Depth 12 -Compress)
if (-not $NoExport) {
    if ($env:GITHUB_OUTPUT) { "key=$key" | Add-Content -LiteralPath $env:GITHUB_OUTPUT -Encoding utf8 }
    if ($env:GITHUB_ENV -and $Component -in @('proxies', 'sdl')) {
        $prefix = if ($Component -eq 'proxies') { 'SHROUDFORGE_PROXY' } else { 'SHROUDFORGE_SDL' }
        $cacheDirectory = Join-Path $root "build/component-cache/$Component"
        "${prefix}_CACHE_KEY=$key" | Add-Content -LiteralPath $env:GITHUB_ENV -Encoding utf8
        "${prefix}_CACHE_DIR=$cacheDirectory" | Add-Content -LiteralPath $env:GITHUB_ENV -Encoding utf8
    }
}
Write-Host "$Component component cache context: $key"
$key
