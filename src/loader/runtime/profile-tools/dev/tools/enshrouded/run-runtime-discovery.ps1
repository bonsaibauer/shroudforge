param(
    [string]$Python = 'python',
    [string]$ClientDirectory = 'C:\Program Files (x86)\Steam\steamapps\common\Enshrouded',
    [string]$ServerDirectory = 'C:\Program Files (x86)\Steam\steamapps\common\EnshroudedServer',
    [ValidateSet('client', 'server', 'both')][string]$Target = 'both',
    [string]$OutputDirectory,
    [switch]$Offline,
    [string]$NativeVerifier,
    [switch]$VerifyBufferAdapters
)
$ErrorActionPreference = 'Stop'
$profileTools = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$runtimeRoot = [IO.Path]::GetFullPath((Join-Path $profileTools '..'))
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $runtimeRoot '../../..'))
if (-not $NativeVerifier) {
    $NativeVerifier = Join-Path $repositoryRoot 'build/native-runtime/tools/kfc-runtime-verify-component-registry.exe'
}
if (-not $Offline -and -not (Test-Path -LiteralPath $NativeVerifier)) {
    throw 'Build the shared production verifier first: cmake --build build/native-runtime --config Release --target kfc-runtime-verify-component-registry'
}
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $profileTools ('devdata/discovery-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
}
foreach ($kind in @('client', 'server')) {
    if ($Target -ne 'both' -and $Target -ne $kind) { continue }
    $directory = if ($kind -eq 'client') { $ClientDirectory } else { $ServerDirectory }
    $stem = if ($kind -eq 'client') { 'enshrouded' } else { 'enshrouded_server' }
    $exe = (Resolve-Path -LiteralPath (Join-Path $directory "$stem.exe")).Path
    $arguments = @((Join-Path $PSScriptRoot 'discover-runtime.py'), $exe,
        '--functions', '--out', (Join-Path $OutputDirectory $kind))
    # A previous profile supplies review anchors only. The scanner checks its PE
    # identity independently and discovers reflection/allocated tables without it.
    $profiles = @(Get-ChildItem -LiteralPath (Join-Path $runtimeRoot "profiles/enshrouded/$kind") -Filter '*.json')
    if ($profiles.Count -eq 1) { $arguments += @('--profile', $profiles[0].FullName) }
    if (-not $Offline) {
        $processes = @(Get-Process -Name $stem -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -and [IO.Path]::GetFullPath($_.Path) -eq $exe })
        if ($processes.Count -ne 1) {
            throw "Expected one running $exe; found $($processes.Count). Use -Offline for EXE analysis only."
        }
        $arguments += @('--pid', [string]$processes[0].Id)
    }
    & $Python @arguments
    if ($LASTEXITCODE -ne 0) { throw "$kind discovery failed with exit code $LASTEXITCODE" }
    if (-not $Offline) {
        $capture = [IO.Path]::GetFullPath((Join-Path $OutputDirectory $kind))
        $components = Get-Content -LiteralPath (Join-Path $capture 'components.json') -Raw | ConvertFrom-Json
        $managers = @($components.registries | ForEach-Object { $_.managers } | Sort-Object distinctLayouts -Descending)
        if ($managers.Count -eq 0) { throw "$kind has no validated live registry manager; the partial capture is retained at $capture" }
        $verification = Join-Path $capture 'provider-verification.json'
        & $NativeVerifier ([string]$processes[0].Id) ($managers[0].address -replace '^0x', '') $verification
        if ($LASTEXITCODE -ne 0) { throw "$kind production registry verification failed" }
        if ($VerifyBufferAdapters) {
            $previousCorpus = $env:SHROUDFORGE_TEST_FUNCTIONS
            Push-Location -LiteralPath $repositoryRoot
            try {
                $env:SHROUDFORGE_TEST_FUNCTIONS = $verification
                & cargo test -p shroudforge-api --lib --offline production_callback_binding_corpus -- --ignored --nocapture
                if ($LASTEXITCODE -ne 0) { throw "$kind Lua callback corpus failed" }
            } finally {
                $env:SHROUDFORGE_TEST_FUNCTIONS = $previousCorpus
                Pop-Location
            }
            & $Python (Join-Path $PSScriptRoot 'verify-buffer-adapters.py') $exe `
                --registry $verification --adapters "$verification.adapters.json" `
                --out (Join-Path $capture 'adapter-verification.json')
            if ($LASTEXITCODE -ne 0) { throw "$kind isolated callback differential verification failed" }
        }
        & $Python (Join-Path $PSScriptRoot 'export-runtime-bindings.py') $exe `
            --capture $capture --out (Join-Path $capture 'runtime-bindings.json') `
            --index (Join-Path $capture 'runtime-index.json')
        if ($LASTEXITCODE -ne 0) { throw "$kind unified runtime catalog export failed" }
    }
}
if ($Target -eq 'both') {
    & $Python (Join-Path $PSScriptRoot 'inspect-runtime.py') compare `
        (Join-Path $OutputDirectory 'client') (Join-Path $OutputDirectory 'server') `
        --out (Join-Path $OutputDirectory 'client-to-server.json')
    if ($LASTEXITCODE -ne 0) { throw 'Client/server comparison failed' }
}
Write-Output "Runtime discovery saved to $OutputDirectory"
