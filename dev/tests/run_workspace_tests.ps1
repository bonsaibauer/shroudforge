<#
.SYNOPSIS
Run bootstrap and Rust workspace tests with a shared Cargo output directory.
.PARAMETER RustOnly
Leave bootstrap tests to the separate native CI job.
.PARAMETER TestGroup
Run All tests by default, or the Main/Diagnostics group in its required CI job.
#>
param(
    [string]$CargoTargetDir = 'target',
    [switch]$RustOnly,
    [ValidateSet('All', 'Main', 'Diagnostics')][string]$TestGroup = 'All'
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$output = Join-Path $root 'build/x64'
$cargoTargetPath = if ([IO.Path]::IsPathRooted($CargoTargetDir)) { [IO.Path]::GetFullPath($CargoTargetDir) } else { [IO.Path]::GetFullPath((Join-Path $root $CargoTargetDir)) }
New-Item -ItemType Directory -Force -Path $output | Out-Null
if (-not $RustOnly) {
    $bootstrapTests = Join-Path $root 'modules/entrypoints/windows-bootstrap/tests/run.ps1'
    & pwsh -NoProfile -File $bootstrapTests
    if ($LASTEXITCODE -ne 0) { throw 'ShroudForge Windows bootstrap tests failed.' }
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
