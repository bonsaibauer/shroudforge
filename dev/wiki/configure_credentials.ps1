[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet('Api', 'Import')]
    [string]$Kind
)

$ErrorActionPreference = 'Stop'
$credentialDirectory = Join-Path $env:LOCALAPPDATA 'ShroudForge'
$credentialPath = if ($Kind -eq 'Api') {
    Join-Path $credentialDirectory 'wiki-api-credential.xml'
}
else {
    Join-Path $credentialDirectory 'wiki-import-credential.xml'
}

New-Item -ItemType Directory -Path $credentialDirectory -Force | Out-Null
$defaultUser = if ($Kind -eq 'Import') { 'Bonsaibauer' } else { $null }
$message = if ($Kind -eq 'Api') {
    'Wiki BotPassword with edit, upload, and pagetranslation rights.'
}
else {
    'Normal Wiki account with translate-import permission. Do not use a BotPassword.'
}
$credentialParameters = @{ Message = $message }
if ($defaultUser) {
    $credentialParameters.UserName = $defaultUser
}
$credential = Get-Credential @credentialParameters

if ($Kind -eq 'Import' -and $credential.UserName.Contains('@')) {
    throw 'Use the normal Wiki username for PO imports, not a BotPassword username.'
}

$credential | Export-Clixml -LiteralPath $credentialPath
Write-Host "Saved a Windows-user-encrypted $Kind Wiki credential to $credentialPath"
