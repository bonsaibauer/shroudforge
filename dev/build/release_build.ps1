<#
.SYNOPSIS
Run the shared ShroudForge build phases with the release packaging options.
.PARAMETER RustOnly
Run only the Rust/frontend phases, including Rust tests. Native CI checks and final
packaging run separately. Without this switch, build and package all components.
#>
param(
    [string]$BuildNumber = $(if ($env:SHROUDFORGE_BUILD_NUMBER) { $env:SHROUDFORGE_BUILD_NUMBER } else { 'dev' }),
    [switch]$SkipTests,
    [switch]$UseInstalledDependencies,
    [switch]$SkipUiBuild,
    [switch]$RustOnly,
    [switch]$SkipProxies,
    [switch]$SkipDiagnostics,
    [switch]$WithoutUpdater,
    [string]$CargoTargetDir = 'target'
)

$ErrorActionPreference = 'Stop'
$buildParameters = @{}
foreach ($name in $PSBoundParameters.Keys) {
    $buildParameters[$name] = $PSBoundParameters[$name]
}
$buildParameters['BuildType'] = 'release'
& (Join-Path $PSScriptRoot 'dev_build.ps1') @buildParameters
