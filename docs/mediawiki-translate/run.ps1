[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [string[]]$ArticleId
)

$ErrorActionPreference = 'Stop'

$buildScript = Join-Path $PSScriptRoot 'build-pages.py'
$preparePoScript = Join-Path $PSScriptRoot 'prepare-localized-po.py'
$populatePoScript = Join-Path $PSScriptRoot 'populate-translated-po.py'
$publishScript = Join-Path $PSScriptRoot 'publish.ps1'
$importScript = Join-Path $PSScriptRoot 'import-po.ps1'

Write-Host 'Preparing PO files from available article translations...'
& python -S $preparePoScript
if ($LASTEXITCODE -ne 0) {
    throw "PO preparation failed with exit code $LASTEXITCODE."
}

Write-Host 'Building PO files from locally translated text...'
& python -S $populatePoScript
if ($LASTEXITCODE -ne 0) {
    throw "PO population failed with exit code $LASTEXITCODE."
}

Write-Host 'Building the MediaWiki source pages and manifest...'
& python -S $buildScript
if ($LASTEXITCODE -ne 0) {
    throw "Page generation failed with exit code $LASTEXITCODE."
}

$childParameters = @{}
if ($ArticleId) {
    $childParameters.ArticleId = $ArticleId
}
if ($WhatIfPreference) {
    $childParameters.WhatIf = $true
}

Write-Host 'Publishing missing pages and setting Modloader priority languages through the Wiki API...'
& $publishScript @childParameters
if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
    throw "Wiki page publishing failed with exit code $LASTEXITCODE."
}

Write-Host 'Importing complete PO files through Special:ImportTranslations...'
& $importScript @childParameters
if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
    throw "PO import failed with exit code $LASTEXITCODE."
}
