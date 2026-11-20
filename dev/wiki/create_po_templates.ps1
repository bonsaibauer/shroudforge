[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
param(
    [Parameter(Mandatory)]
    [ValidatePattern('^page-.+$')]
    [string]$Group,

    [Parameter(Mandatory)]
    [string]$WorkspacePath,

    [ValidateSet('de-at', 'de-ch', 'de', 'es', 'fr', 'hi', 'it', 'ja', 'nl', 'pl', 'sv', 'zh')]
    [string[]]$Locale,

    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$apiUrl = 'https://wiki.shroudforge.net/api.php'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$workspaceRoot = [IO.Path]::GetFullPath((Join-Path $root 'target\wiki'))
$workspaceCandidate = if ([IO.Path]::IsPathRooted($WorkspacePath)) { $WorkspacePath } else { Join-Path $root $WorkspacePath }
$workspace = [IO.Path]::GetFullPath($workspaceCandidate)
$separator = [IO.Path]::DirectorySeparatorChar
if (-not $workspace.StartsWith($workspaceRoot.TrimEnd($separator) + $separator, [StringComparison]::OrdinalIgnoreCase)) {
    throw "WorkspacePath must resolve below target/wiki: $workspace"
}

$credentialPath = Join-Path $env:LOCALAPPDATA 'ShroudForge\wiki-api-credential.xml'
if (-not (Test-Path -LiteralPath $credentialPath)) {
    throw "Wiki API credential is missing. Run dev/wiki/configure_credentials.ps1 -Kind Api first."
}
$credential = Import-Clixml -LiteralPath $credentialPath
if ($credential -isnot [PSCredential]) {
    throw 'The encrypted Wiki API credential is not a PSCredential file.'
}

$session = [Microsoft.PowerShell.Commands.WebRequestSession]::new()
function New-QueryString {
    param([hashtable]$Parameters)
    $pairs = foreach ($key in $Parameters.Keys) {
        [Uri]::EscapeDataString([string]$key) + '=' + [Uri]::EscapeDataString([string]$Parameters[$key])
    }
    return $pairs -join '&'
}
function Invoke-WikiGet {
    param([hashtable]$Parameters)
    $query = New-QueryString $Parameters
    Invoke-RestMethod -Uri "$apiUrl`?$query" -WebSession $session
}

$loginToken = (Invoke-WikiGet @{ action = 'query'; meta = 'tokens'; type = 'login'; format = 'json' }).query.tokens.logintoken
$login = Invoke-RestMethod -Uri $apiUrl -Method Post -WebSession $session -Body @{
    action = 'login'
    lgname = $credential.UserName
    lgpassword = $credential.GetNetworkCredential().Password
    lgtoken = $loginToken
    format = 'json'
}
if ($login.login.result -ne 'Success') {
    throw "Wiki API login failed with result '$($login.login.result)'. The encrypted credential was not printed."
}

$groups = (Invoke-WikiGet @{
    action = 'query'
    meta = 'messagegroups'
    mgformat = 'flat'
    mgprop = 'id|prioritylangs|sourcelanguage'
    format = 'json'
}).query.messagegroups
$messageGroup = $groups | Where-Object { $_.id -eq $Group } | Select-Object -First 1
if (-not $messageGroup) {
    throw "Translate group was not found: $Group. Publish and mark the English source first."
}
if ($messageGroup.sourcelanguage -ne 'en-gb') {
    throw "Translate group '$Group' has unexpected source language '$($messageGroup.sourcelanguage)'."
}
$priorityLanguages = @($messageGroup.prioritylangs | ForEach-Object { ([string]$_).ToLowerInvariant() })
if ($priorityLanguages.Count -eq 0) {
    throw "Translate group '$Group' has no priority languages."
}
$targetLanguages = if ($Locale) {
    @($Locale | ForEach-Object { $_.ToLowerInvariant() })
}
else {
    $priorityLanguages
}
$unsupported = @($targetLanguages | Where-Object { $_ -notin $priorityLanguages })
if ($unsupported.Count -gt 0) {
    throw "These locales are not priority languages for '$Group': $($unsupported -join ', ')"
}

$sourceMessages = [Collections.Generic.List[object]]::new()
$messageParameters = @{
    action = 'query'
    list = 'messagecollection'
    mcgroup = $Group
    mclanguage = 'en-gb'
    mcprop = 'translation'
    mclimit = 'max'
    format = 'json'
}
do {
    $response = Invoke-WikiGet $messageParameters
    if ($response.error) {
        throw "Could not read live source messages for '$Group': $($response.error.code) $($response.error.info)"
    }
    foreach ($message in @($response.query.messagecollection)) {
        $sourceMessages.Add($message)
    }
    if ($response.continue -and $response.continue.mccontinue) {
        $messageParameters.mccontinue = [string]$response.continue.mccontinue
    }
    else {
        $messageParameters.Remove('mccontinue')
        break
    }
} while ($true)

if ($sourceMessages.Count -eq 0) {
    throw "Translate group '$Group' has no English message units."
}
if ([string]$sourceMessages[0].key -notmatch '/Page_display_title$') {
    throw "The first live message in '$Group' is not its page title."
}
$seenKeys = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach ($message in $sourceMessages) {
    if (-not $seenKeys.Add([string]$message.key)) {
        throw "Translate group '$Group' returned duplicate message context '$($message.key)'."
    }
}

if (-not (Test-Path -LiteralPath $workspace)) {
    if ($PSCmdlet.ShouldProcess($workspace, 'Create temporary translation workspace')) {
        New-Item -ItemType Directory -Path $workspace -Force | Out-Null
    }
}
if (-not (Test-Path -LiteralPath $workspace -PathType Container)) {
    Write-Host "WhatIf: would create translation workspace $workspace"
    return
}

$pageTitle = $Group.Substring('page-'.Length)
$createdAt = [DateTime]::UtcNow.ToString('yyyy-MM-dd HH:mm:ss+0000', [Globalization.CultureInfo]::InvariantCulture)
foreach ($language in $targetLanguages) {
    $poPath = Join-Path $workspace "$language.po"
    if ((Test-Path -LiteralPath $poPath) -and -not $Force) {
        Write-Host "Existing PO file left unchanged: $poPath. Use -Force only to replace it."
        continue
    }

    $languageHeader = $language.Replace('-', '_')
    $lines = [Collections.Generic.List[string]]::new()
    $lines.Add('msgid ""')
    $lines.Add('msgstr ""')
    $lines.Add('"PO-Revision-Date: ' + $createdAt + '\n"')
    $lines.Add('"POT-Creation-Date: ' + $createdAt + '\n"')
    $lines.Add('"Content-Type: text/plain; charset=UTF-8\n"')
    $lines.Add('"Content-Transfer-Encoding: 8bit\n"')
    $lines.Add('"Language: ' + $languageHeader + '\n"')
    $lines.Add('"X-Generator: ShroudForge dev/wiki\n"')
    $lines.Add('"X-Language-Code: ' + $language + '\n"')
    $lines.Add('"X-Message-Group: ' + $Group + '\n"')

    foreach ($message in $sourceMessages) {
        $contextText = ConvertTo-Json -InputObject ([string]$message.key) -Compress
        $sourceText = ConvertTo-Json -InputObject ([string]$message.translation) -Compress
        $lines.Add('')
        $lines.Add('#')
        $lines.Add("msgctxt $contextText")
        $lines.Add("msgid $sourceText")
        $lines.Add('msgstr ""')
    }

    $poText = $lines -join "`n"
    if ($PSCmdlet.ShouldProcess($poPath, 'Write temporary PO template')) {
        [IO.File]::WriteAllText($poPath, $poText + "`n", [Text.UTF8Encoding]::new($false))
        Write-Host "Created live-context PO template: $poPath"
    }
}
