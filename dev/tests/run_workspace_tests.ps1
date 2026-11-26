<#
.SYNOPSIS
Run bootstrap and Rust workspace tests, or the configured native runtime fixtures.
.PARAMETER RustOnly
Leave bootstrap tests to the separate native CI job.
.PARAMETER TestGroup
Run Rust All/Main/Diagnostics, or Native after the canonical native build.
#>
param(
    [string]$CargoTargetDir = 'target',
    [switch]$RustOnly,
    [ValidateSet('All', 'Main', 'Diagnostics', 'Native')][string]$TestGroup = 'All'
)

$ErrorActionPreference = 'Stop'
if ($RustOnly -and $TestGroup -eq 'Native') { throw 'Native tests cannot use RustOnly.' }
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$output = Join-Path $root 'build/x64'
$cargoTargetPath = if ([IO.Path]::IsPathRooted($CargoTargetDir)) { [IO.Path]::GetFullPath($CargoTargetDir) } else { [IO.Path]::GetFullPath((Join-Path $root $CargoTargetDir)) }
New-Item -ItemType Directory -Force -Path $output | Out-Null
if (-not $RustOnly) {
    $bootstrapTests = Join-Path $root 'modules/entrypoints/windows-bootstrap/tests/run.ps1'
    & pwsh -NoProfile -File $bootstrapTests
    if ($LASTEXITCODE -ne 0) { throw 'ShroudForge Windows bootstrap tests failed.' }
    if ($TestGroup -eq 'Native') {
        $nativeBuild = Join-Path $root 'build/native-runtime'
        $targets = @('shroudforge-test-native-plugin-abi', 'kfc-runtime-test-map-frame',
            'kfc-runtime-test-map-service-lifetime', 'kfc-runtime-test-overlay-gpu',
            'kfc-runtime-test-resource', 'kfc-runtime-test-ecs-read', 'kfc-runtime-test-service',
            'kfc-runtime-test-service-lifecycle', 'kfc-runtime-test-executable-span-validation')
        & cmake --build $nativeBuild --config Release --target @targets --parallel 2
        if ($LASTEXITCODE -ne 0) { throw 'Native runtime fixture build failed.' }
        foreach ($target in $targets) {
            & (Join-Path $nativeBuild "modules/game-runtime/native/Release/$target.exe")
            if ($LASTEXITCODE -ne 0) { throw "Native runtime test failed: $target" }
        }
        foreach ($mode in @('--prepare-failure', '--unsupported-map')) {
            & (Join-Path $nativeBuild 'modules/game-runtime/native/Release/kfc-runtime-test-service-lifecycle.exe') $mode
            if ($LASTEXITCODE -ne 0) { throw "Native service lifecycle test failed: $mode" }
        }
        foreach ($mode in @('--terminal-swap', '--terminal-device')) {
            & (Join-Path $nativeBuild 'modules/game-runtime/native/Release/kfc-runtime-test-overlay-gpu.exe') $mode
            if ($LASTEXITCODE -ne 0) { throw "GPU terminal test failed: $mode" }
        }
    }
}

if ($TestGroup -in @('All', 'Main')) {
    & cargo test --manifest-path (Join-Path $root 'Cargo.toml') --locked --release --workspace --exclude shroudforge-runtime-diagnostics --target-dir $cargoTargetPath --timings
    if ($LASTEXITCODE -ne 0) { throw 'ShroudForge workspace tests failed.' }
}

# Keep mlua/serialize from diagnostic test helpers out of the product feature graph.
if ($TestGroup -in @('All', 'Diagnostics')) {
    & cargo test --manifest-path (Join-Path $root 'Cargo.toml') --locked --release -p shroudforge-runtime-diagnostics --target-dir $cargoTargetPath --timings
    if ($LASTEXITCODE -ne 0) { throw 'ShroudForge runtime diagnostics tests failed.' }
}
