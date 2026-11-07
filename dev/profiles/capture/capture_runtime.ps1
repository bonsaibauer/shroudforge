param(
    [string]$Python = 'python',
    [string]$ClientDirectory = 'C:\Program Files (x86)\Steam\steamapps\common\Enshrouded',
    [string]$ServerDirectory = 'C:\Program Files (x86)\Steam\steamapps\common\EnshroudedServer',
    [ValidateSet('client', 'server', 'both')][string]$Target = 'both',
    [string]$ClientBaselineProfile,
    [string]$ServerBaselineProfile,
    [string]$OutputDirectory,
    [switch]$Offline,
    [string]$NativeVerifier,
    [string]$ResourceExporter,
    [switch]$VerifyBufferAdapters,
    [Alias('InspectModOrigins')][switch]$VerifyRuntimeOrigins,
    [switch]$RunCallbackCorpusTests
)
$ErrorActionPreference = 'Stop'
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$runtimeRoot = Join-Path $repositoryRoot 'modules/game-runtime'
$pythonRoot = Join-Path $repositoryRoot 'modules/runtime-diagnostics/diagnostics-jobs'
if (-not $NativeVerifier) {
    $NativeVerifier = Join-Path $repositoryRoot 'build/native-runtime/tools/kfc-runtime-verify-component-registry.exe'
}
if (-not $Offline -and -not (Test-Path -LiteralPath $NativeVerifier)) {
    throw 'Build the shared production verifier first: cmake --build build/native-runtime --config Release --target kfc-runtime-verify-component-registry'
}
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $repositoryRoot ('dev/profiles/captures/discovery-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
}
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
if ($VerifyRuntimeOrigins) {
    if (-not $ResourceExporter) {
        $ResourceExporter = Join-Path $repositoryRoot 'target/debug/examples/export_kfc_resources.exe'
    }
    if (-not (Test-Path -LiteralPath $ResourceExporter)) {
        throw 'Build the exporter explicitly first: cargo build -p shroudforge-api --example export_kfc_resources --offline, or supply -ResourceExporter.'
    }
}
foreach ($kind in @('client', 'server')) {
    if ($Target -ne 'both' -and $Target -ne $kind) { continue }
    $directory = if ($kind -eq 'client') { $ClientDirectory } else { $ServerDirectory }
    $stem = if ($kind -eq 'client') { 'enshrouded' } else { 'enshrouded_server' }
    $exe = (Resolve-Path -LiteralPath (Join-Path $directory "$stem.exe")).Path
    # Reuse the inspection package's PE/KFC readers instead of inferring identity
    # from filenames, modification times, or the newest profile in a directory.
    Push-Location -LiteralPath $pythonRoot
    try {
        $identityJson = & $Python -c @'
import json, sys
from game_inspection.inspect.read_pe_image import PE
from game_inspection.capture.discover_runtime_build import build_version
pe = PE(sys.argv[1])
print(json.dumps(dict(image=pe.identity(), kfcFormat=build_version(pe)[1])))
'@ $exe
    } finally {
        Pop-Location
    }
    if ($LASTEXITCODE -ne 0) { throw "$kind executable identity inspection failed" }
    $identity = $identityJson | ConvertFrom-Json
    $arguments = @($exe, '--functions', '--out', (Join-Path $OutputDirectory $kind))
    $profiles = @(Get-ChildItem -LiteralPath (Join-Path $runtimeRoot "profiles/enshrouded/$kind") -Filter '*.json')
    $profileRecords = @($profiles | ForEach-Object {
        $document = Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json
        $buildId = 0L
        $prefix = [regex]::Escape("enshrouded-$kind-")
        if ($document.id -match "^$prefix([1-9][0-9]*)$") {
            [long]::TryParse($Matches[1], [ref]$buildId) | Out-Null
        }
        [pscustomobject]@{ File = $_; Document = $document; BuildId = $buildId }
    })
    $matchingProfiles = @($profileRecords | Where-Object {
        $_.Document.target -eq $identity.image.target -and
        $_.Document.kfcFormat -eq $identity.kfcFormat -and
        $_.Document.image.sha256 -eq $identity.image.sha256 -and
        $_.Document.image.timestamp -eq $identity.image.timestamp -and
        $_.Document.image.size -eq $identity.image.size
    } | ForEach-Object { $_.File })
    if ($matchingProfiles.Count -gt 1) {
        throw "$kind has multiple exact profiles: $($matchingProfiles.FullName -join ', '). Resolve the ambiguity before capturing."
    }
    $selectedProfile = $null
    $selectionReason = 'no-exact-profile-or-explicit-baseline'
    if ($matchingProfiles.Count -eq 1) {
        $selectedProfile = $matchingProfiles[0].FullName
        $selectionReason = 'exact-image-profile'
    } else {
        $baseline = if ($kind -eq 'client') { $ClientBaselineProfile } else { $ServerBaselineProfile }
        if ($baseline) {
            $selectedProfile = (Resolve-Path -LiteralPath $baseline).Path
            $document = Get-Content -LiteralPath $selectedProfile -Raw | ConvertFrom-Json
            if ($document.target -ne $identity.image.target -or $document.kfcFormat -ne $identity.kfcFormat) {
                throw "$kind baseline must describe $($identity.image.target) and KFC$($identity.kfcFormat)"
            }
            $selectionReason = 'explicit-same-target-baseline'
        } else {
            $baselines = @($profileRecords | Where-Object {
                $_.Document.target -eq $identity.image.target -and
                $_.Document.kfcFormat -eq $identity.kfcFormat -and $_.BuildId -gt 0
            } | Sort-Object BuildId -Descending)
            if ($baselines.Count -gt 1 -and $baselines[0].BuildId -eq $baselines[1].BuildId) {
                throw "$kind has multiple latest review baselines for build $($baselines[0].BuildId). Supply an explicit baseline."
            }
            if ($baselines.Count -gt 0) {
                $selectedProfile = $baselines[0].File.FullName
                $selectionReason = 'latest-same-target-kfc-review-baseline'
            }
        }
    }
    if ($selectedProfile) {
        $arguments += @('--profile', $selectedProfile)
        Write-Output "$kind profile review anchors: $selectedProfile ($selectionReason)"
    } else {
        $parameter = if ($kind -eq 'client') { '-ClientBaselineProfile' } else { '-ServerBaselineProfile' }
        Write-Warning "$kind has no exact profile. Supply $parameter for explicit review anchors. Capturing profile-independent evidence only."
    }
    if (-not $Offline) {
        $processes = @(Get-Process -Name $stem -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -and [IO.Path]::GetFullPath($_.Path) -eq $exe })
        if ($processes.Count -ne 1) {
            throw "Expected one running $exe; found $($processes.Count). Use -Offline for EXE analysis only."
        }
        $arguments += @('--pid', [string]$processes[0].Id)
    }
    Push-Location -LiteralPath $pythonRoot
    try {
        & $Python -m game_inspection.capture.discover_runtime_build @arguments
    } finally {
        Pop-Location
    }
    if ($LASTEXITCODE -ne 0) { throw "$kind discovery failed with exit code $LASTEXITCODE" }
    $capture = [IO.Path]::GetFullPath((Join-Path $OutputDirectory $kind))
    # Persist the selected baseline even when the catalog contains many profiles.
    # It supplies review anchors and never grants target execution approval.
    $capturedIdentity = (Get-Content -LiteralPath (Join-Path $capture 'reflection.json') -Raw | ConvertFrom-Json).image
    if ($capturedIdentity.sha256 -ne $identity.image.sha256 -or
        $capturedIdentity.timestamp -ne $identity.image.timestamp -or
        $capturedIdentity.size -ne $identity.image.size -or
        $capturedIdentity.target -ne $identity.image.target) {
        throw "$kind executable changed during capture; partial evidence is retained at $capture"
    }
    [ordered]@{
        schemaVersion = 1
        image = $capturedIdentity
        selection = $selectionReason
        profile = $selectedProfile
        exactImageMatch = ($matchingProfiles.Count -eq 1)
        purpose = 'review-anchors-only'
        productionApproval = $false
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $capture 'profile-selection.json') -Encoding utf8
    if ($VerifyRuntimeOrigins) {
        & $ResourceExporter $exe (Join-Path $capture 'resources.json')
        if ($LASTEXITCODE -ne 0) { throw "$kind KFC resource inspection failed" }
        if ($matchingProfiles.Count -eq 1) {
            Push-Location -LiteralPath $pythonRoot
            try {
                & $Python -m game_inspection.verify.verify_runtime_origins $exe --profile $matchingProfiles[0].FullName `
                    --functions (Join-Path $capture 'functions.json') --resources (Join-Path $capture 'resources.json') `
                    --out (Join-Path $capture 'mod-origins.json')
            } finally {
                Pop-Location
            }
            if ($LASTEXITCODE -ne 0) { throw "$kind mod origin audit failed" }
        } else { Write-Warning "$kind has no unique exact profile; original resources and function descriptors are captured, mod interventions remain unverified" }
    }
    if (-not $Offline) {
        $capture = [IO.Path]::GetFullPath((Join-Path $OutputDirectory $kind))
        $components = Get-Content -LiteralPath (Join-Path $capture 'components.json') -Raw | ConvertFrom-Json
        $managers = @($components.registries | ForEach-Object { $_.managers } | Sort-Object distinctLayouts -Descending)
        if ($managers.Count -eq 0) { throw "$kind has no validated live registry manager; the partial capture is retained at $capture" }
        $verification = Join-Path $capture 'provider-verification.json'
        $verifierArguments = @([string]$processes[0].Id, ($managers[0].address -replace '^0x', ''), $verification)
        if ($matchingProfiles.Count -eq 1) { $verifierArguments += $matchingProfiles[0].FullName }
        & $NativeVerifier @verifierArguments
        if ($LASTEXITCODE -ne 0) { throw "$kind production registry verification failed" }
        if ($RunCallbackCorpusTests) {
            & (Join-Path $PSScriptRoot '..\checks\run_callback_corpus_tests.ps1') -RegistryCapture $verification
        }
        if ($VerifyBufferAdapters) {
            Push-Location -LiteralPath $pythonRoot
            try {
                & $Python -m game_inspection.checks.verify_component_copy_adapters $exe `
                    --registry $verification --adapters "$verification.adapters.json" `
                    --out (Join-Path $capture 'adapter-verification.json')
            } finally {
                Pop-Location
            }
            if ($LASTEXITCODE -ne 0) { throw "$kind isolated callback differential verification failed" }
        }
        Push-Location -LiteralPath $pythonRoot
        try {
            & $Python -m game_inspection.capture.export_runtime_bindings $exe `
                --capture $capture --out (Join-Path $capture 'runtime-bindings.json') `
                --index (Join-Path $capture 'runtime-index.json')
        } finally {
            Pop-Location
        }
        if ($LASTEXITCODE -ne 0) { throw "$kind unified runtime catalog export failed" }
    }
}
if ($Target -eq 'both') {
    Push-Location -LiteralPath $pythonRoot
    try {
        & $Python -m game_inspection.compare.compare_runtime_builds `
            (Join-Path $OutputDirectory 'client') (Join-Path $OutputDirectory 'server') `
            --out (Join-Path $OutputDirectory 'client-to-server.json')
    } finally {
        Pop-Location
    }
    if ($LASTEXITCODE -ne 0) { throw 'Client/server comparison failed' }
}
Write-Output "Runtime discovery saved to $OutputDirectory"
