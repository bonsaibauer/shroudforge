[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [string[]]$ArticleId,
    [switch]$ConfigureCredential
)

$ErrorActionPreference = 'Stop'

$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$manifestPath = Join-Path $PSScriptRoot 'publish-manifest.json'
$apiCredentialPath = Join-Path $env:LOCALAPPDATA 'ShroudForge\wiki-api-credential.xml'
$importCredentialPath = Join-Path $env:LOCALAPPDATA 'ShroudForge\wiki-import-credential.xml'
$apiUrl = 'https://wiki.shroudforge.net/api.php'
$importUrl = 'https://wiki.shroudforge.net/index.php/Special:ImportTranslations'

if ($ConfigureCredential) {
    $directory = Split-Path -Parent $importCredentialPath
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    $credential = Get-Credential -UserName 'Bonsaibauer' -Message 'Normales Wiki-Konto mit translate-import-Recht. Kein Bot-Passwort eingeben.'
    if ($credential.UserName -match '@') {
        throw 'Use the normal Wiki username for this credential, not the BotPasswords username.'
    }
    $credential | Export-Clixml -LiteralPath $importCredentialPath
    Write-Host "Saved a Windows-user-encrypted Wiki import credential to $importCredentialPath"
    return
}

# A dry run authenticates with the already configured API bot. A real import
# needs a normal Wiki account because BotPasswords cannot use Special: pages.
if ($WhatIfPreference) {
    $credentialPath = $apiCredentialPath
    if (-not (Test-Path -LiteralPath $credentialPath)) {
        throw "Encrypted Wiki API credential not found: $credentialPath"
    }
}
else {
    $credentialPath = $importCredentialPath
    if (-not (Test-Path -LiteralPath $credentialPath)) {
        $directory = Split-Path -Parent $credentialPath
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
        $credential = Get-Credential -UserName 'Bonsaibauer' -Message 'Normales Wiki-Konto mit translate-import-Recht. Kein Bot-Passwort eingeben.'
        if ($credential.UserName -match '@') {
            throw 'Use the normal Wiki username for this credential, not the BotPasswords username.'
        }
        $credential | Export-Clixml -LiteralPath $credentialPath
        Write-Host "Saved a Windows-user-encrypted Wiki import credential to $credentialPath"
    }
}

if (-not (Test-Path -LiteralPath $manifestPath)) {
    throw "Publish manifest not found. Run build-pages.py first: $manifestPath"
}
$credential = Import-Clixml -LiteralPath $credentialPath
if ($credential -isnot [PSCredential]) {
    throw 'The encrypted Wiki import credential is not a PSCredential file.'
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding utf8 | ConvertFrom-Json
$articles = @($manifest.articles)
if ($ArticleId) {
    $selected = @($articles | Where-Object { $_.id -in $ArticleId })
    $notFound = @($ArticleId | Where-Object { $_ -notin $selected.id })
    if ($notFound.Count -gt 0) {
        throw "Unknown article IDs in the publish manifest: $($notFound -join ', ')"
    }
    $articles = $selected
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

function Get-InputAttribute {
    param(
        [string]$Tag,
        [string]$Name
    )

    $pattern = [Regex]::Escape($Name) + '\s*=\s*(?:"([^"]*)"|''([^'']*)''|([^\s>]+))'
    $match = [Regex]::Match($Tag, $pattern, [Text.RegularExpressions.RegexOptions]::IgnoreCase)
    if (-not $match.Success) {
        return $null
    }
    foreach ($index in 1..3) {
        if ($match.Groups[$index].Success) {
            return [Net.WebUtility]::HtmlDecode($match.Groups[$index].Value)
        }
    }
    return $null
}

function Get-InputValue {
    param(
        [string]$Html,
        [string]$Name
    )

    foreach ($match in [Regex]::Matches($Html, '(?is)<input\b[^>]*>')) {
        $tag = $match.Value
        if ((Get-InputAttribute -Tag $tag -Name 'name') -eq $Name) {
            return Get-InputAttribute -Tag $tag -Name 'value'
        }
    }
    return $null
}

function ConvertFrom-PoQuotedString {
    param([string]$Value)

    try {
        return ConvertFrom-Json -InputObject ('"' + $Value + '"') -ErrorAction Stop
    }
    catch {
        throw 'A PO string contains an escape sequence that the importer cannot verify safely.'
    }
}

function Get-ImportActions {
    param([string]$Html)

    $names = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($match in [Regex]::Matches($Html, '(?is)<input\b[^>]*>')) {
        $tag = $match.Value
        $type = Get-InputAttribute -Tag $tag -Name 'type'
        $value = Get-InputAttribute -Tag $tag -Name 'value'
        $name = Get-InputAttribute -Tag $tag -Name 'name'
        if ($type -eq 'radio' -and $value -eq 'import' -and $name) {
            [void]$names.Add($name)
        }
    }
    return @($names)
}

function Get-PoEntries {
    param([string]$Text)

    $entries = [Collections.Generic.List[object]]::new()
    $context = $null
    $translation = ''
    $activeField = ''

    foreach ($line in ($Text -split "`r?`n")) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            if ($context) {
                $entries.Add([pscustomobject]@{ Context = $context; Translation = $translation })
            }
            $context = $null
            $translation = ''
            $activeField = ''
            continue
        }

        if ($line -match '^msgctxt\s+"(.*)"$') {
            $context = $matches[1]
            $activeField = 'msgctxt'
            continue
        }
        if ($line -match '^msgstr(?:\[\d+\])?\s+"(.*)"$') {
            if ($context) {
                $translation += ConvertFrom-PoQuotedString -Value $matches[1]
            }
            $activeField = 'msgstr'
            continue
        }
        if ($line -match '^msg(?:id|id_plural)\s') {
            $activeField = 'msgid'
            continue
        }
        if ($line -match '^"(.*)"$' -and $activeField -eq 'msgstr' -and $context) {
            $translation += ConvertFrom-PoQuotedString -Value $matches[1]
        }
    }

    if ($context) {
        $entries.Add([pscustomobject]@{ Context = $context; Translation = $translation })
    }
    return @($entries)
}

function Assert-NoImportError {
    param([string]$Html)

    if ($Html -match 'translate-import-err-|mw-message-box-error|class=["'']errorbox["'']|Login required') {
        $plain = [Net.WebUtility]::HtmlDecode(($Html -replace '(?is)<script\b.*?</script>|<style\b.*?</style>|<[^>]+>', ' '))
        $plain = [Regex]::Replace($plain, '\s+', ' ').Trim()
        throw "Special:ImportTranslations returned an error: $plain"
    }
}

$loginToken = (Invoke-WikiGet @{ action = 'query'; meta = 'tokens'; type = 'login'; format = 'json' }).query.tokens.logintoken
if ($WhatIfPreference) {
    $login = Invoke-RestMethod -Uri $apiUrl -Method Post -WebSession $session -Body @{
        action = 'login'
        lgname = $credential.UserName
        lgpassword = $credential.GetNetworkCredential().Password
        lgtoken = $loginToken
        format = 'json'
    }
    if ($login.login.result -ne 'Success') {
        throw "Wiki API bot login failed with result '$($login.login.result)'. The encrypted credential was not printed."
    }
}
else {
    $login = Invoke-RestMethod -Uri $apiUrl -Method Post -WebSession $session -Body @{
        action = 'clientlogin'
        username = $credential.UserName
        password = $credential.GetNetworkCredential().Password
        loginreturnurl = 'https://wiki.shroudforge.net/'
        logintoken = $loginToken
        format = 'json'
    }
    if ($login.clientlogin.status -ne 'PASS') {
        $reason = $login.clientlogin.messagecode
        if (-not $reason) { $reason = $login.clientlogin.status }
        throw "Normal Wiki login did not complete ($reason). Two-factor, CAPTCHA, or another interactive login step may require an interactive login. The password was not printed."
    }
}

$userInfo = (Invoke-WikiGet @{ action = 'query'; meta = 'userinfo'; uiprop = 'rights'; format = 'json' }).query.userinfo
if (-not $WhatIfPreference -and 'translate-import' -notin $userInfo.rights) {
    throw "Normal Wiki account '$($userInfo.name)' lacks the translate-import right. The BotPasswords credential cannot access Special:ImportTranslations."
}
$csrfToken = (Invoke-WikiGet @{ action = 'query'; meta = 'tokens'; type = 'csrf'; format = 'json' }).query.tokens.csrftoken
$report = [Collections.Generic.List[object]]::new()
$expectedTranslationPages = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)

foreach ($article in $articles) {
    $poFiles = @($article.poFiles)
    if ($poFiles.Count -eq 0 -and $article.po) {
        $poFiles = @([string]$article.po)
    }
    if ($poFiles.Count -eq 0) {
        $report.Add([pscustomobject]@{ ArticleId = $article.id; Group = $article.group; Status = 'No PO files' })
        continue
    }

    foreach ($poFile in $poFiles) {
        $poPath = Join-Path $root ([string]$poFile)
        if (-not (Test-Path -LiteralPath $poPath)) {
            throw "PO file listed in the manifest was not found: $poPath"
        }

        $poText = Get-Content -LiteralPath $poPath -Raw -Encoding utf8
        $languageMatch = [Regex]::Match($poText, '(?m)"X-Language-Code:\s*([A-Za-z0-9-]+)\\n"')
        if (-not $languageMatch.Success) {
            throw "PO language header is missing or invalid: $poPath"
        }
        $language = $languageMatch.Groups[1].Value.ToLowerInvariant()
        if ($language -notin @($article.priorityLanguages | ForEach-Object { ([string]$_).ToLowerInvariant() })) {
            throw "PO language '$language' is not a priority language for '$($article.group)'."
        }
        if (-not $poText.Contains('"X-Message-Group: ' + [string]$article.group + '\n"')) {
            throw "PO message group does not match '$($article.group)': $poPath"
        }

        $entries = @(Get-PoEntries -Text $poText)
        if ($entries.Count -eq 0) {
            throw "PO file has no msgctxt entries: $poPath"
        }
        $translatedEntries = @($entries | Where-Object { -not [string]::IsNullOrEmpty($_.Translation) })
        if ($translatedEntries.Count -eq 0) {
            $report.Add([pscustomobject]@{
                ArticleId = $article.id
                Group = $article.group
                Language = $language
                Entries = $entries.Count
                Status = 'Needs translation'
            })
            continue
        }
        if ($translatedEntries.Count -ne $entries.Count) {
            throw "PO file is incomplete for '$($article.group)' / '$language'. Import it after every unit has a translation: $poPath"
        }

        $groupQuery = Invoke-WikiGet @{
            action = 'query'
            list = 'messagecollection'
            mcgroup = [string]$article.group
            mclanguage = $language
            mcprop = 'translation'
            format = 'json'
        }
        if ($groupQuery.error) {
            throw "Could not read Translate group '$($article.group)' in '$language': $($groupQuery.error.code)"
        }
        $liveMessages = @($groupQuery.query.messagecollection)
        $liveByKey = @{}
        foreach ($message in $liveMessages) {
            $liveByKey[[string]$message.key] = $message
        }
        $unknownContexts = @($entries | Where-Object { -not $liveByKey.ContainsKey([string]$_.Context) })
        if ($unknownContexts.Count -gt 0) {
            throw "PO contexts are not in live group '$($article.group)': $(($unknownContexts.Context) -join ', ')"
        }

        if (-not $PSCmdlet.ShouldProcess($poPath, "Upload and import $language PO for $($article.group)")) {
            $report.Add([pscustomobject]@{
                ArticleId = $article.id
                Group = $article.group
                Language = $language
                Status = 'WhatIf'
                Entries = $entries.Count
            })
            continue
        }

        $page = Invoke-WebRequest -Uri $importUrl -WebSession $session
        if ($page.Content -notmatch 'id=["'']mw-translate-import["'']') {
            throw "The normal Wiki account did not receive the Special:ImportTranslations upload form for '$($article.group)'."
        }

        $preview = Invoke-WebRequest -Uri $importUrl -Method Post -WebSession $session -Form @{
            title = 'Special:ImportTranslations'
            token = $csrfToken
            'upload-local' = Get-Item -LiteralPath $poPath
        }
        $html = $preview.Content
        Assert-NoImportError -Html $html

        $submittedLanguage = Get-InputValue -Html $html -Name 'language'
        if ($submittedLanguage -and $submittedLanguage.ToLowerInvariant() -ne $language) {
            throw "Translate import preview selected '$submittedLanguage' instead of '$language' for '$($article.group)'."
        }

        $round = 0
        while ($true) {
            $processFlag = Get-InputValue -Html $html -Name 'process'
            if (-not $processFlag) {
                break
            }
            if ($processFlag -ne '1') {
                throw "Unexpected process value on Special:ImportTranslations for '$($article.group)' / '$language'."
            }
            $round++
            if ($round -gt 20) {
                throw "Translate import for '$($article.group)' / '$language' did not finish after 20 processing batches."
            }

            $form = @{
                title = 'Special:ImportTranslations'
                token = $csrfToken
                process = '1'
                language = $language
            }
            foreach ($fieldName in (Get-ImportActions -Html $html)) {
                $form[$fieldName] = 'import'
            }
            $response = Invoke-WebRequest -Uri $importUrl -Method Post -WebSession $session -Body $form
            $html = $response.Content
            Assert-NoImportError -Html $html
        }

        $verifyQuery = Invoke-WikiGet @{
            action = 'query'
            list = 'messagecollection'
            mcgroup = [string]$article.group
            mclanguage = $language
            mcprop = 'translation'
            format = 'json'
        }
        $verifyMessages = @($verifyQuery.query.messagecollection)
        $verifyByKey = @{}
        foreach ($message in $verifyMessages) {
            $verifyByKey[[string]$message.key] = $message
        }
        $notImported = @($translatedEntries | Where-Object {
            -not $verifyByKey.ContainsKey([string]$_.Context) -or
            [string]::IsNullOrWhiteSpace([string]$verifyByKey[[string]$_.Context].translation) -or
            [string]$verifyByKey[[string]$_.Context].translation -cne [string]$_.Translation
        })
        if ($notImported.Count -gt 0) {
            throw "Import finished but $($notImported.Count) PO entries do not match the saved translations in '$($article.group)' / '$language': $(($notImported.Context) -join ', ')"
        }

        $report.Add([pscustomobject]@{
            ArticleId = $article.id
            Group = $article.group
            Language = $language
            Entries = $entries.Count
            ImportedEntries = $translatedEntries.Count
            Status = 'Imported through Special:ImportTranslations and verified'
        })
        [void]$expectedTranslationPages.Add("$($article.title)/$language")
    }
}

$report | Format-Table -AutoSize

if (-not $WhatIfPreference -and $expectedTranslationPages.Count -gt 0) {
    $pageTitles = @($expectedTranslationPages)
    $missingPages = [Collections.Generic.List[string]]::new()
    for ($offset = 0; $offset -lt $pageTitles.Count; $offset += 50) {
        $end = [Math]::Min($offset + 49, $pageTitles.Count - 1)
        $batch = @($pageTitles[$offset..$end])
        $pageQuery = Invoke-WikiGet @{
            action = 'query'
            prop = 'info'
            titles = ($batch -join '|')
            format = 'json'
        }
        foreach ($page in $pageQuery.query.pages.PSObject.Properties.Value) {
            if ($page.missing -ne $null) {
                $missingPages.Add([string]$page.title)
            }
        }
    }

    if ($missingPages.Count -gt 0) {
        $statistics = (Invoke-WikiGet @{
            action = 'query'
            meta = 'siteinfo'
            siprop = 'statistics'
            format = 'json'
        }).query.statistics
        throw "All PO entries were saved, but $($missingPages.Count) of $($expectedTranslationPages.Count) translated pages are not rendered yet. MediaWiki reports approximately $($statistics.jobs) queued jobs. The wiki host must process Translate render jobs with maintenance/run.php runJobs before those pages become available. First inspect the job types with maintenance/run.php showJobs --group. Missing examples: $(($missingPages | Select-Object -First 8) -join ', ')"
    }
}
