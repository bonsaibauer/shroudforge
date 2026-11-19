[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [Parameter(Mandatory)]
    [string]$WorkspacePath,

    [string]$EditSummary = 'Publish English source article for ShroudForge Translate',

    [ValidateSet('de-at', 'de-ch', 'de', 'es', 'fr', 'hi', 'it', 'ja', 'nl', 'pl', 'sv', 'zh')]
    [string[]]$PriorityLanguages = @('de-at', 'de-ch', 'de', 'es', 'fr', 'hi', 'it', 'ja', 'nl', 'pl', 'sv', 'zh'),

    [switch]$UpdateExisting
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
if (-not (Test-Path -LiteralPath $workspace -PathType Container)) {
    throw "Temporary article workspace does not exist: $workspace"
}

$wikitextPath = Join-Path $workspace 'en.wikitext'
if (-not (Test-Path -LiteralPath $wikitextPath -PathType Leaf)) {
    throw "English source Wikitext was not found: $wikitextPath"
}
$wikitext = Get-Content -LiteralPath $wikitextPath -Raw -Encoding utf8
if ($wikitext -notmatch '(?s)<translate>.*?</translate>') {
    throw 'The English source must contain a complete <translate> block.'
}
if ($wikitext -notmatch '\[\[Category:guide:Shroudforge\]\]') {
    throw 'The page must include [[Category:guide:Shroudforge]].'
}
$metadataMatch = [Regex]::Match($wikitext, '(?s)<!--\s*ShroudForge Modloader Wiki Post(?<metadata>.*?)-->')
if (-not $metadataMatch.Success) {
    throw 'The ShroudForge Modloader Wiki Post metadata block is missing.'
}
$metadata = $metadataMatch.Groups['metadata'].Value
$titleMatch = [Regex]::Match($metadata, '(?im)^title:\s*(.+?)\s*$')
$languageMatch = [Regex]::Match($metadata, '(?im)^language:\s*(.+?)\s*$')
if (-not $titleMatch.Success -or [string]::IsNullOrWhiteSpace($titleMatch.Groups[1].Value)) {
    throw 'The metadata title is missing.'
}
if (-not $languageMatch.Success -or $languageMatch.Groups[1].Value -ine 'en-GB') {
    throw 'The public Wiki source language must be en-GB.'
}
$title = $titleMatch.Groups[1].Value.Trim()
$groupId = "page-$title"

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
function Invoke-WikiPost {
    param([hashtable]$Parameters)
    Invoke-RestMethod -Uri $apiUrl -Method Post -Body $Parameters -WebSession $session
}
function Test-WikiMissing {
    param($Page)
    return $Page.PSObject.Properties.Name -contains 'missing'
}

$loginToken = (Invoke-WikiGet @{ action = 'query'; meta = 'tokens'; type = 'login'; format = 'json' }).query.tokens.logintoken
$login = Invoke-WikiPost @{
    action = 'login'
    lgname = $credential.UserName
    lgpassword = $credential.GetNetworkCredential().Password
    lgtoken = $loginToken
    format = 'json'
}
if ($login.login.result -ne 'Success') {
    throw "Wiki API login failed with result '$($login.login.result)'. The encrypted credential was not printed."
}
$userInfo = (Invoke-WikiGet @{ action = 'query'; meta = 'userinfo'; uiprop = 'rights'; format = 'json' }).query.userinfo
$requiredRights = @('edit', 'createpage', 'upload', 'pagetranslation')
$missingRights = @($requiredRights | Where-Object { $_ -notin $userInfo.rights })
if ($missingRights.Count -gt 0) {
    throw "Wiki bot '$($userInfo.name)' lacks required rights: $($missingRights -join ', ')"
}
$csrfToken = (Invoke-WikiGet @{ action = 'query'; meta = 'tokens'; type = 'csrf'; format = 'json' }).query.tokens.csrftoken

$pageQuery = Invoke-WikiGet @{ action = 'query'; prop = 'info'; titles = $title; format = 'json' }
$page = $pageQuery.query.pages.PSObject.Properties.Value | Select-Object -First 1
$pageMissing = Test-WikiMissing $page
$shouldEdit = $pageMissing -or $UpdateExisting

if ($shouldEdit) {
    $fileReferences = [Regex]::Matches($wikitext, '\[\[(?:File|Image):([^|\]\r\n]+)')
    $fileNames = @($fileReferences | ForEach-Object { $_.Groups[1].Value.Trim() } | Select-Object -Unique)
    foreach ($fileName in $fileNames) {
        if ($fileName.Contains('/') -or $fileName.Contains('\') -or $fileName.Contains('..')) {
            throw "Only plain MediaWiki filenames are supported in this temporary article: $fileName"
        }
        $filePath = Join-Path $workspace $fileName
        $fileQuery = Invoke-WikiGet @{
            action = 'query'
            prop = 'imageinfo'
            iiprop = 'url'
            titles = "File:$fileName"
            format = 'json'
        }
        $filePage = $fileQuery.query.pages.PSObject.Properties.Value | Select-Object -First 1
        $fileMissing = Test-WikiMissing $filePage
        if (-not $fileMissing) {
            Write-Host "Existing Wiki file left unchanged: File:$fileName"
            continue
        }
        if (-not (Test-Path -LiteralPath $filePath -PathType Leaf)) {
            throw "Referenced Wiki file is not on the Wiki and is missing from the temporary workspace: $filePath"
        }
        if ($PSCmdlet.ShouldProcess("File:$fileName", 'Upload article image')) {
            $upload = Invoke-RestMethod -Uri $apiUrl -Method Post -WebSession $session -Form @{
                action = 'upload'
                filename = $fileName
                file = Get-Item -LiteralPath $filePath
                comment = "Upload image for Translate page $title"
                token = $csrfToken
                format = 'json'
            }
            if ($upload.error) {
                throw "Wiki image upload failed for 'File:$fileName': $($upload.error.code) $($upload.error.info)"
            }
            if ($upload.upload.result -ne 'Success') {
                $warningText = @($upload.upload.warnings.PSObject.Properties.Name) -join ', '
                throw "Wiki image upload failed for 'File:$fileName' with result '$($upload.upload.result)' $warningText"
            }
        }
    }
}

$revisionId = if ($pageMissing) { $null } else { [string]$page.lastrevid }
$created = $false
if ($shouldEdit -and $PSCmdlet.ShouldProcess($title, $(if ($pageMissing) { 'Create English source page' } else { 'Update English source page' }))) {
    $editParameters = @{
        action = 'edit'
        title = $title
        text = $wikitext
        summary = $EditSummary
        token = $csrfToken
        format = 'json'
    }
    if ($pageMissing) {
        $editParameters.createonly = '1'
    }
    else {
        $editParameters.baserevid = [string]$page.lastrevid
    }
    $edit = Invoke-WikiPost $editParameters
    if ($edit.error) {
        throw "Wiki page edit failed: $($edit.error.code) $($edit.error.info)"
    }
    $revisionId = [string]$edit.edit.newrevid
    $created = $pageMissing
}
elseif (-not $shouldEdit) {
    Write-Host "Existing English source left unchanged: $title"
}

$marked = $false
if ($revisionId) {
    if ($PSCmdlet.ShouldProcess($title, 'Mark English source for translation and set priority languages')) {
        $mark = Invoke-WikiPost @{
            action = 'markfortranslation'
            title = $title
            revid = $revisionId
            translatetitle = 'yes'
            prioritylanguages = (@($PriorityLanguages | ForEach-Object { $_.ToLowerInvariant() }) -join '|')
            forcepriority = '0'
            transclusion = 'yes'
            token = $csrfToken
            format = 'json'
        }
        if ($mark.error) {
            throw "Translate marking failed: $($mark.error.code) $($mark.error.info)"
        }
        $marked = $true
    }
}

$groups = (Invoke-WikiGet @{
    action = 'query'
    meta = 'messagegroups'
    mgformat = 'flat'
    mgprop = 'id|prioritylangs|priorityforce|sourcelanguage'
    format = 'json'
}).query.messagegroups
$group = $groups | Where-Object { $_.id -eq $groupId } | Select-Object -First 1
if ($marked -and -not $group) {
    throw "Translate group '$groupId' was not returned after marking '$title'."
}
if ($group -and $group.sourcelanguage -ne 'en-gb') {
    throw "Translate group '$groupId' has unexpected source language '$($group.sourcelanguage)'."
}
$expectedLocales = @($PriorityLanguages | ForEach-Object { $_.ToLowerInvariant() })
$reportedLocales = @($group.prioritylangs | ForEach-Object { ([string]$_).ToLowerInvariant() })
$missingLocales = @($expectedLocales | Where-Object { $_ -notin $reportedLocales })
if ($marked -and $missingLocales.Count -gt 0) {
    throw "Translate group '$groupId' is missing priority languages: $($missingLocales -join ', ')"
}

[PSCustomObject]@{
    Page = $title
    Group = $groupId
    SourceLanguage = if ($group) { $group.sourcelanguage } else { 'pending' }
    PriorityLanguages = if ($group) { @($group.prioritylangs) -join ', ' } else { $expectedLocales -join ', ' }
    Created = $created
    MarkedForTranslation = $marked
    Status = if ($marked) { 'Published and verified' } elseif ($WhatIfPreference) { 'WhatIf' } else { 'No changes requested' }
} | Format-List
