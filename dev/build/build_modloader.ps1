param([string]$CargoTargetDir = 'target', [switch]$UseInstalledDependencies, [switch]$SkipUiBuild)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$output = Join-Path $root 'build/x64'
New-Item -ItemType Directory -Force -Path $output | Out-Null
$cargoTargetPath = if ([IO.Path]::IsPathRooted($CargoTargetDir)) { [IO.Path]::GetFullPath($CargoTargetDir) } else { [IO.Path]::GetFullPath((Join-Path $root $CargoTargetDir)) }
& (Join-Path $PSScriptRoot 'build_modloader_frontend.ps1') -UseInstalledDependencies:$UseInstalledDependencies -SkipUiBuild:$SkipUiBuild
& cargo build --locked --manifest-path (Join-Path $root 'Cargo.toml') --release --workspace --target-dir $cargoTargetPath
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
