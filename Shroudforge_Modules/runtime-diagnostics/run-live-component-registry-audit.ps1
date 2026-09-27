[CmdletBinding()]
param(
    [uint32]$ProcessId = 0,
    [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$buildRoot = Join-Path $repoRoot 'build\native-diagnostics'
$tool = Join-Path $buildRoot 'Release\audit-live-component-registry.exe'
$catalog = Join-Path $repoRoot 'site\data\enshrouded-1076226---game38-branches-ea-update-08-2026-06-29t10-27-39-052394z\runtime-ecs-types.json'
$profile = Join-Path $repoRoot 'Shroudforge_Modloader\kfc-runtime\compatibility\profiles\enshrouded-client-1076226.json'

if (-not (Test-Path -LiteralPath $catalog)) { throw "ECS type catalog not found: $catalog" }
if (-not (Test-Path -LiteralPath $profile)) { throw "Build profile not found: $profile" }

if ($ProcessId -eq 0) {
    $game = Get-Process -Name enshrouded -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -and [IO.Path]::GetFullPath($_.Path) -match '\\enshrouded\.exe$' } |
        Select-Object -First 1
    if (-not $game) {
        throw 'Enshrouded is not running. Start it, load into a world, then run this script again.'
    }
    $ProcessId = [uint32]$game.Id
}

if (-not (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue)) {
    throw "No running process has PID $ProcessId."
}

& cmake --build $buildRoot --config Release --target audit-live-component-registry
if ($LASTEXITCODE -ne 0) { throw "CMake failed to build audit-live-component-registry (exit $LASTEXITCODE)." }

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot 'build\component-registry-audit'
}
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$log = Join-Path $OutputDirectory "registry-audit-$ProcessId-$stamp.log"
$mappingFile = Join-Path $OutputDirectory "registry-candidates-$ProcessId-$stamp.tsv"

Write-Host "Scanning Enshrouded PID $ProcessId; this reads process memory and does not modify the game."
Write-Host "Full output: $log"
& $tool $ProcessId $catalog $profile 2>&1 | Tee-Object -FilePath $log
if ($LASTEXITCODE -ne 0) { throw "Registry audit exited with code $LASTEXITCODE. See $log" }

$mappings = Get-Content -LiteralPath $log | Where-Object { $_ -match '^new_mapping\t' }
if ($mappings) {
    Set-Content -LiteralPath $mappingFile -Value $mappings -Encoding utf8
    Write-Host "Candidate mappings: $mappingFile"
} else {
    Write-Warning 'No additional table candidate passed the known-profile cross-check. Keep the full log for the next reverse-engineering pass.'
}
