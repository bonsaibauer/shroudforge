[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [string[]]$ArticleId
)

$ErrorActionPreference = 'Stop'

$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$manifestPath = Join-Path $PSScriptRoot 'publish-manifest.json'
$credentialPath = Join-Path $env:LOCALAPPDATA 'ShroudForge\wiki-api-credential.xml'
$apiUrl = 'https://wiki.shroudforge.net/api.php'

if (-not (Test-Path -LiteralPath $manifestPath)) {
    throw "Publish manifest not found. Run build-pages.py first: $manifestPath"
}
if (-not (Test-Path -LiteralPath $credentialPath)) {
    throw "Encrypted Wiki API credential not found: $credentialPath"
}

$credential = Import-Clixml -LiteralPath $credentialPath
if ($credential -isnot [PSCredential]) {
    throw 'The encrypted Wiki API credential is not a PSCredential file.'
}

$session = [Microsoft.PowerShell.Commands.WebRequestSession]::new()

function New-QueryString {
    param([hashtable]$Parameters)

    $pairs = foreach ($key in $Parameters.Keys) {
        $name = [Uri]::EscapeDataString([string]$key)
        $value = [Uri]::EscapeDataString([string]$Parameters[$key])
        [string]::Format('{0}={1}', $name, $value)
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

$manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding utf8 | ConvertFrom-Json
$articles = @($manifest.articles)
if ($ArticleId) {
    $articles = @($articles | Where-Object { $_.id -in $ArticleId })
    $notFound = @($ArticleId | Where-Object { $_ -notin $articles.id })
    if ($notFound.Count -gt 0) {
        throw "Unknown article IDs in the publish manifest: $($notFound -join ', ')"
    }
}

$csrfToken = (Invoke-WikiGet @{ action = 'query'; meta = 'tokens'; type = 'csrf'; format = 'json' }).query.tokens.csrftoken
$report = foreach ($article in $articles) {
    $title = [string]$article.title
    $pageQuery = Invoke-WikiGet @{
        action = 'query'
        prop = 'info'
        titles = $title
        format = 'json'
    }
    $page = $pageQuery.query.pages.PSObject.Properties.Value | Select-Object -First 1
    # MediaWiki represents the `missing` marker as an empty XML-style
    # attribute. Check whether the property exists instead of coercing its
    # empty value to Boolean false in PowerShell.
    $pageMissing = $page.PSObject.Properties.Name -contains 'missing'
    $created = $false
    $markApplied = $false

    if ($pageMissing) {
        $assetPath = Join-Path $root ([string]$article.asset)
        if (-not (Test-Path -LiteralPath $assetPath)) {
            throw "Illustration is missing for '$title': $assetPath"
        }

        $assetTitle = 'File:' + [string]$article.assetTitle
        $fileQuery = Invoke-WikiGet @{
            action = 'query'
            prop = 'imageinfo'
            iiprop = 'url'
            titles = $assetTitle
            format = 'json'
        }
        $filePage = $fileQuery.query.pages.PSObject.Properties.Value | Select-Object -First 1
        $fileMissing = $filePage.PSObject.Properties.Name -contains 'missing'
        if ($fileMissing) {
            if ($PSCmdlet.ShouldProcess($assetTitle, 'Upload illustration')) {
                $upload = Invoke-RestMethod -Uri $apiUrl -Method Post -WebSession $session -Form @{
                    action = 'upload'
                    filename = [string]$article.assetTitle
                    file = Get-Item -LiteralPath $assetPath
                    comment = 'Upload illustration for Translate page ' + $title
                    token = $csrfToken
                    format = 'json'
                }
                if ($upload.upload.result -ne 'Success') {
                    $warningText = @($upload.upload.warnings.PSObject.Properties.Name) -join ', '
                    throw "Illustration upload failed for '$title' with result '$($upload.upload.result)' $warningText"
                }
            }
        }

        $wikitextPath = Join-Path $root ([string]$article.wikitext)
        if (-not (Test-Path -LiteralPath $wikitextPath)) {
            throw "Generated English Wikitext is missing for '$title': $wikitextPath"
        }
        $wikitext = Get-Content -LiteralPath $wikitextPath -Raw -Encoding utf8
        if ($PSCmdlet.ShouldProcess($title, 'Create English source page')) {
            $edit = Invoke-WikiPost @{
                action = 'edit'
                title = $title
                text = $wikitext
                summary = 'Create English source page for ShroudForge Translate'
                createonly = '1'
                token = $csrfToken
                format = 'json'
            }
            if ($edit.error) {
                throw "Page creation failed for '$title' with API error '$($edit.error.code)'."
            }
            $page = [pscustomobject]@{ title = $title; pageid = $edit.edit.pageid; lastrevid = $edit.edit.newrevid }
            $created = $true
        }
    }
    else {
        Write-Host "Existing source page left unchanged: $title"
    }

    if ($page.lastrevid) {
        $markResult = $null
        if ($PSCmdlet.ShouldProcess($title, 'Mark for translation and set German priority')) {
            $markResult = Invoke-WikiPost @{
                action = 'markfortranslation'
                title = $title
                revid = [string]$page.lastrevid
                translatetitle = 'yes'
                prioritylanguages = (@($article.priorityLanguages) -join '|')
                forcepriority = '0'
                transclusion = 'yes'
                token = $csrfToken
                format = 'json'
            }
            if ($markResult.error) {
                throw "Translate marking failed for '$title' with API error '$($markResult.error.code)'."
            }
            $markApplied = $true
        }

        $groups = (Invoke-WikiGet @{
            action = 'query'
            meta = 'messagegroups'
            mgformat = 'flat'
            mgprop = 'id|prioritylangs|priorityforce|sourcelanguage'
            format = 'json'
        }).query.messagegroups
        $group = $groups | Where-Object { $_.id -eq [string]$article.group } | Select-Object -First 1
        if ($markApplied -and -not $group) {
            throw "Translate group '$($article.group)' was not returned after marking '$title'."
        }
        if ($group -and $group.sourcelanguage -ne 'en-gb') {
            throw "Translate group '$($article.group)' has unexpected source language '$($group.sourcelanguage)'."
        }
        $expectedPriorityLanguages = @($article.priorityLanguages | ForEach-Object { ([string]$_).ToLowerInvariant() })
        $reportedPriorityLanguages = @($group.prioritylangs | ForEach-Object { ([string]$_).ToLowerInvariant() })
        $missingPriorityLanguages = @($expectedPriorityLanguages | Where-Object { $_ -notin $reportedPriorityLanguages })
        if ($group -and $missingPriorityLanguages.Count -gt 0) {
            throw "Translate group '$($article.group)' is missing priority languages: $($missingPriorityLanguages -join ', ')"
        }
        if ($group -and $group.priorityforce) {
            throw "Translate group '$($article.group)' restricts translations to priority languages, which was not requested."
        }

        [pscustomobject]@{
            ArticleId = $article.id
            Page = $title
            Group = $article.group
            SourceLanguage = if ($group) { $group.sourcelanguage } else { 'pending' }
            PriorityLanguages = if ($group) { @($group.prioritylangs) -join ', ' } else { 'pending' }
            Created = $created
            Status = if ($markApplied) { 'published and verified' } else { 'WhatIf' }
        }
    }
    else {
        [pscustomobject]@{ ArticleId = $article.id; Page = $title; Group = $article.group; Created = $created; Status = 'WhatIf' }
    }
}

$report | Format-Table -AutoSize
