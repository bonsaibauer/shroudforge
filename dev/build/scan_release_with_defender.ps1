param(
    [string]$ArchiveDirectory = 'build',
    [string]$ReportPath = 'defender-release-report.json',
    [ValidateRange(1, 5)][int]$MaxScanAttempts = 4,
    [ValidateRange(1, 300)][int]$RetryDelaySeconds = 30
)

$ErrorActionPreference = 'Stop'
$scanStartedAt = [DateTime]::UtcNow
$runId = if ($env:GITHUB_RUN_ID) { $env:GITHUB_RUN_ID } else { [Guid]::NewGuid().ToString('N') }
$temporaryRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [IO.Path]::GetTempPath() }
$scanRoot = Join-Path $temporaryRoot "shroudforge-release-scan-$runId"
$report = [ordered]@{
    scanner = 'Microsoft Defender Antivirus'
    target = 'Release ZIP files and extracted contents'
    status = 'error'
    scan_started_at = $scanStartedAt.ToString('o')
    scan_completed_at = $null
    archive_count = 0
    prepared_file_count = 0
    scanned_file_count = 0
    scan_attempt_count = 0
    scan_attempt_errors = @()
    signature_update_status = 'not_attempted'
    signature_update_message = $null
    signature_update_attempts = @()
    metadata = [ordered]@{}
    metadata_warnings = @()
    detection_count = 0
    detections = @()
    error = $null
}

function Get-ReleaseDetectionDetails {
    param([string[]]$ResourceTargets)

    $matchedDetections = @(Get-MpThreatDetection -ErrorAction Stop | Where-Object {
        $resourceText = (@($_.Resources) | ForEach-Object { [string]$_ }) -join "`n"
        $matchedTarget = $false
        foreach ($target in $ResourceTargets) {
            if ($resourceText.IndexOf($target, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
                $matchedTarget = $true
                break
            }
        }
        $matchedTarget
    })

    @($matchedDetections | ForEach-Object {
        [ordered]@{
            threat_name = [string]$_.ThreatName
            initial_detection_time = if ($_.InitialDetectionTime) { $_.InitialDetectionTime.ToUniversalTime().ToString('o') } else { $null }
            resources = @($_.Resources | ForEach-Object { [string]$_ })
        }
    })
}

function Get-MpCmdRunPath {
    $platformRoot = Join-Path $env:ProgramData 'Microsoft\Windows Defender\Platform'
    $platformCommand = Get-ChildItem -LiteralPath $platformRoot -Directory -ErrorAction SilentlyContinue |
        Sort-Object -Property Name -Descending |
        ForEach-Object { Join-Path $_.FullName 'MpCmdRun.exe' } |
        Where-Object { Test-Path -LiteralPath $_ } |
        Select-Object -First 1
    if ($platformCommand) {
        return $platformCommand
    }

    $legacyCommand = Join-Path $env:ProgramFiles 'Windows Defender\MpCmdRun.exe'
    if (Test-Path -LiteralPath $legacyCommand) {
        return $legacyCommand
    }

    $null
}

function Restore-DefenderAvailability {
    param([string]$Reason)

    if ($env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
        throw "Automatic Defender recovery is restricted to GitHub-hosted runners. Original scan error: $Reason"
    }

    $mpCmdRun = Get-MpCmdRunPath
    if (-not $mpCmdRun) {
        throw "Could not find MpCmdRun.exe to re-enable Microsoft Defender. Original scan error: $Reason"
    }

    Write-Host 'Attempting to re-enable Microsoft Defender on the hosted runner.'
    $recoveryOutput = (& $mpCmdRun -WdEnable 2>&1 | Out-String).Trim()
    $recoveryExitCode = $LASTEXITCODE
    if ($recoveryExitCode -ne 0) {
        throw "MpCmdRun.exe -WdEnable exited with code $recoveryExitCode. $recoveryOutput"
    }

    $service = Get-Service -Name WinDefend -ErrorAction Stop
    if ($service.Status -ne 'Running') {
        Start-Service -Name WinDefend -ErrorAction Stop
        $service = Get-Service -Name WinDefend -ErrorAction Stop
        $service.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Running, [TimeSpan]::FromSeconds(20))
    }
}

try {
    $archives = @(Get-ChildItem -LiteralPath $ArchiveDirectory -Filter 'shroudforge-*.zip' -File)
    if ($archives.Count -eq 0) {
        throw "No release ZIPs were found in '$ArchiveDirectory'."
    }
    $report.archive_count = $archives.Count
    $report.archives = @($archives | ForEach-Object { $_.Name })
    $detectionTargets = @($scanRoot) + @($archives | ForEach-Object { $_.FullName })

    New-Item -ItemType Directory -Path $scanRoot -Force | Out-Null
    foreach ($archive in $archives) {
        $scanArchive = Join-Path $scanRoot $archive.Name
        Copy-Item -LiteralPath $archive.FullName -Destination $scanArchive
        $destination = Join-Path $scanRoot ([IO.Path]::GetFileNameWithoutExtension($archive.Name))
        Expand-Archive -LiteralPath $archive.FullName -DestinationPath $destination
    }

    $report.prepared_file_count = @(Get-ChildItem -LiteralPath $scanRoot -File -Recurse).Count
    $scanSucceeded = $false
    $lastScanError = $null
    for ($attempt = 1; $attempt -le $MaxScanAttempts; $attempt++) {
        $report.scan_attempt_count = $attempt
        Write-Host "Microsoft Defender scan attempt $attempt of $MaxScanAttempts."
        $attemptServiceStatus = $null
        $attemptServiceStartType = $null
        $attemptEngineVersion = $null
        try {
            $signatureAttempt = [ordered]@{
                attempt = $attempt
                status = 'warning'
                message = $null
            }
            try {
                Update-MpSignature -ErrorAction Stop
                $report.signature_update_status = 'success'
                $report.signature_update_message = $null
                $signatureAttempt.status = 'success'
                Write-Host 'Microsoft Defender signature update completed.'
            } catch {
                $report.signature_update_status = 'warning'
                $report.signature_update_message = $_.Exception.Message
                $signatureAttempt.message = $_.Exception.Message
                Write-Warning "Microsoft Defender signature update failed. Scanning with the signatures already present. $($_.Exception.Message)"
            }
            $report.signature_update_attempts += $signatureAttempt

            $service = Get-Service -Name WinDefend -ErrorAction Stop
            $attemptServiceStatus = [string]$service.Status
            $attemptServiceStartType = [string]$service.StartType
            $report.metadata.defender_service_status = [string]$service.Status
            $report.metadata.defender_service_start_type = [string]$service.StartType
            if ($service.Status -ne 'Running') {
                throw "Microsoft Defender Antivirus service WinDefend is '$($service.Status)' (start type '$($service.StartType)')."
            }

            $computerStatus = Get-MpComputerStatus -ErrorAction Stop
            $attemptEngineVersion = [string]$computerStatus.AMEngineVersion
            $report.metadata.engine_version = [string]$computerStatus.AMEngineVersion
            $report.metadata.signature_version = [string]$computerStatus.AntivirusSignatureVersion
            $report.metadata.signature_last_updated = if ($computerStatus.AntivirusSignatureLastUpdated) { $computerStatus.AntivirusSignatureLastUpdated.ToUniversalTime().ToString('o') } else { $null }
            $report.metadata.real_time_protection_enabled = $computerStatus.RealTimeProtectionEnabled
            if ($report.metadata.engine_version -eq '0.0.0.0') {
                throw 'Microsoft Defender reported engine version 0.0.0.0, so the scan engine is unavailable.'
            }

            try {
                $preference = Get-MpPreference -ErrorAction Stop
                $report.metadata.cloud_protection_level = $preference.MAPSReporting
            } catch {
                $report.metadata_warnings += "Could not read Defender protection settings: $($_.Exception.Message)"
            }

            Start-MpScan -ScanType CustomScan -ScanPath $scanRoot -ErrorAction Stop
            $report.scanned_file_count = $report.prepared_file_count
            $scanSucceeded = $true
            break
        } catch {
            $lastScanError = $_.Exception.Message
            $attemptError = [ordered]@{
                attempt = $attempt
                error = $lastScanError
                defender_service_status = $attemptServiceStatus
                defender_service_start_type = $attemptServiceStartType
                engine_version = $attemptEngineVersion
            }
            $report.scan_attempt_errors += $attemptError

            if ($attempt -lt $MaxScanAttempts) {
                Write-Warning "Microsoft Defender attempt $attempt of $MaxScanAttempts failed: $lastScanError"
                try {
                    Restore-DefenderAvailability -Reason $lastScanError
                    $attemptError.recovery = 'Microsoft Defender was re-enabled successfully.'
                } catch {
                    $attemptError.recovery = $_.Exception.Message
                    $report.metadata_warnings += "Could not restore Defender before retry $($attempt + 1): $($_.Exception.Message)"
                    Write-Warning "Could not restore Microsoft Defender before retry $($attempt + 1): $($_.Exception.Message)"
                }

                Write-Host "Waiting $RetryDelaySeconds seconds before Defender retry $($attempt + 1) of $MaxScanAttempts."
                Start-Sleep -Seconds $RetryDelaySeconds
            }
        }
    }

    if (-not $scanSucceeded) {
        throw "Microsoft Defender scan failed after $($report.scan_attempt_count) attempt(s): $lastScanError"
    }

    # The scan directory is unique to this workflow run. Include every detection
    # that references it or a downloaded ZIP, including real-time detections.
    $report.detections = @(Get-ReleaseDetectionDetails -ResourceTargets $detectionTargets)
    $report.detection_count = $report.detections.Count
    $report.scan_completed_at = [DateTime]::UtcNow.ToString('o')
    if ($report.detection_count -gt 0) {
        $report.status = 'findings'
        Write-Host "Microsoft Defender reported $($report.detection_count) detection(s) in the release contents."
    } else {
        $report.status = 'clean'
        Write-Host "Microsoft Defender found no malware in $($archives.Count) release ZIP(s)."
    }
} catch {
    $failureMessage = $_.Exception.Message
    $report.status = 'error'
    $report.error = $failureMessage
    $report.scan_completed_at = [DateTime]::UtcNow.ToString('o')
    if ($detectionTargets) {
        try {
            $report.detections = @(Get-ReleaseDetectionDetails -ResourceTargets $detectionTargets)
            $report.detection_count = $report.detections.Count
            if ($report.detection_count -gt 0) {
                $report.status = 'findings'
            }
        } catch {
            $report.metadata_warnings += "Could not retrieve Defender detections after the scan error: $($_.Exception.Message)"
        }
    }
    Write-Error "Microsoft Defender release scan failed: $failureMessage" -ErrorAction Continue
} finally {
    $reportDirectory = Split-Path -Parent $ReportPath
    if ($reportDirectory) {
        New-Item -ItemType Directory -Path $reportDirectory -Force | Out-Null
    }
    $report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $ReportPath -Encoding utf8NoBOM
}

if ($report.status -ne 'clean') {
    exit 1
}
