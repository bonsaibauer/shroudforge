<#
.SYNOPSIS
Build, test and package ShroudForge through the shared development and release phases.
.PARAMETER RustOnly
Build the frontend, Windows proxies and Rust workspace and run Rust tests. Separate
CI jobs may own proxies and diagnostics through the explicit switches below.
.PARAMETER SkipProxies
With RustOnly, leave the proxy build to the separate verified proxy CI job.
.PARAMETER SkipDiagnostics
With RustOnly, leave diagnostics tests to the separate required diagnostics CI job.
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
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$actions = Join-Path $repositoryRoot 'dev\build'
$cargoTargetPath = if ([IO.Path]::IsPathRooted($CargoTargetDir)) { [IO.Path]::GetFullPath($CargoTargetDir) } else { [IO.Path]::GetFullPath((Join-Path $repositoryRoot $CargoTargetDir)) }
if ($BuildNumber -notmatch '^[A-Za-z0-9._-]+$') { throw 'BuildNumber contains invalid path characters.' }
if (($SkipProxies -or $SkipDiagnostics) -and -not $RustOnly) {
    throw 'SkipProxies and SkipDiagnostics require RustOnly. Full builds always include these phases.'
}

function Invoke-BuildPhase {
    param([string]$Name, [scriptblock]$Action)

    $stopwatch = [Diagnostics.Stopwatch]::StartNew()
    Write-Host "[build-timing] START $Name"
    try {
        & $Action
    } finally {
        $stopwatch.Stop()
        Write-Host ("[build-timing] END {0} elapsed_seconds={1:N1}" -f $Name, $stopwatch.Elapsed.TotalSeconds)
    }
}

function Start-BuildPhaseJob {
    param([string]$Name, [string]$ScriptPath, [hashtable]$Parameters = @{})

    $parametersJson = ConvertTo-Json -InputObject $Parameters -Compress
    Start-Job -Name $Name -ArgumentList $Name, $ScriptPath, $parametersJson -ScriptBlock {
        param($PhaseName, $PhaseScriptPath, $SerializedParameters)

        $ErrorActionPreference = 'Stop'
        $stopwatch = [Diagnostics.Stopwatch]::StartNew()
        Write-Output "[build-timing] START $PhaseName"
        try {
            $phaseParameters = ConvertFrom-Json -InputObject $SerializedParameters -AsHashtable
            & $PhaseScriptPath @phaseParameters
            if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) { throw "$PhaseName exited with code $LASTEXITCODE." }
        } catch {
            $stopwatch.Stop()
            Write-Output ("[build-timing] END {0} status=failed elapsed_seconds={1:N1}" -f $PhaseName, $stopwatch.Elapsed.TotalSeconds)
            throw
        }

        $stopwatch.Stop()
        Write-Output ("[build-timing] END {0} status=success elapsed_seconds={1:N1}" -f $PhaseName, $stopwatch.Elapsed.TotalSeconds)
    }
}

function Wait-BuildPhaseJobs {
    param([System.Management.Automation.Job[]]$Jobs)

    try {
        $remainingJobs = @($Jobs)
        while ($remainingJobs.Count -gt 0) {
            $job = Wait-Job -Job $remainingJobs -Any
            Receive-Job -Job $job -ErrorAction Continue 2>&1 | ForEach-Object {
                Write-Host "[$($job.Name)] $_"
            }
            $remainingJobs = @($remainingJobs | Where-Object Id -ne $job.Id)
        }

        $failedJobs = @($Jobs | Where-Object State -ne 'Completed')
        if ($failedJobs.Count -gt 0) {
            throw "Build phase(s) failed: $($failedJobs.Name -join ', ')"
        }
    } finally {
        Remove-Job -Job $Jobs -Force -ErrorAction SilentlyContinue
    }
}

Invoke-BuildPhase -Name 'prepare-parser' -Action {
    & (Join-Path $repositoryRoot 'dev\parser\prepare_parser.ps1')
}
$version = (Get-Content -LiteralPath (Join-Path $repositoryRoot 'VERSION') -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw "VERSION must use MAJOR.MINOR.PATCH: $version" }

$parallelTasksStopwatch = [Diagnostics.Stopwatch]::StartNew()
$parallelTasks = @(
    (Start-BuildPhaseJob -Name 'modloader-frontend' -ScriptPath (Join-Path $actions 'build_modloader_frontend.ps1') -Parameters @{
        UseInstalledDependencies = $UseInstalledDependencies.IsPresent
        SkipUiBuild = $SkipUiBuild.IsPresent
    })
    if (-not $SkipProxies) {
        (Start-BuildPhaseJob -Name 'windows-proxies' -ScriptPath (Join-Path $actions 'build_windows_proxies.ps1') -Parameters @{
            CargoTargetDir = $cargoTargetPath
        })
    }
    if (-not $RustOnly) {
        (Start-BuildPhaseJob -Name 'kfc-runtime' -ScriptPath (Join-Path $actions 'build_kfc_runtime.ps1'))
        (Start-BuildPhaseJob -Name 'windows-bootstrap' -ScriptPath (Join-Path $actions 'build_windows_bootstrap.ps1'))
    }
)
try {
    Wait-BuildPhaseJobs -Jobs $parallelTasks
} finally {
    $parallelTasksStopwatch.Stop()
    Write-Host ("[build-timing] END parallel-foundation elapsed_seconds={0:N1}" -f $parallelTasksStopwatch.Elapsed.TotalSeconds)
}

if (-not $SkipTests) {
    Invoke-BuildPhase -Name 'workspace-tests' -Action {
        $testGroup = if ($SkipDiagnostics) { 'Main' } else { 'All' }
        & (Join-Path $repositoryRoot 'dev\tests\run_workspace_tests.ps1') -CargoTargetDir $cargoTargetPath -RustOnly:$RustOnly -TestGroup $testGroup
    }
}
Invoke-BuildPhase -Name 'modloader-workspace-build' -Action {
    & (Join-Path $actions 'build_modloader.ps1') -CargoTargetDir $cargoTargetPath -UseInstalledDependencies -SkipUiBuild
}
if (-not $RustOnly) {
    Invoke-BuildPhase -Name 'package-release' -Action {
        & (Join-Path $actions 'package_release.ps1') -BuildNumber $BuildNumber -Version $version -WithoutUpdater:$WithoutUpdater
    }
}
