[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [string[]]$PoPath,
    [switch]$ConfigureCredential
)

$ErrorActionPreference = 'Stop'

$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$workspaceRoot = [IO.Path]::GetFullPath((Join-Path $root 'target\wiki'))
$apiCredentialPath = Join-Path $env:LOCALAPPDATA 'ShroudForge\wiki-api-credential.xml'
$importCredentialPath = Join-Path $env:LOCALAPPDATA 'ShroudForge\wiki-import-credential.xml'
$apiUrl = 'https://wiki.shroudforge.net/api.php'
$importUrl = 'https://wiki.shroudforge.net/index.php/Special:ImportTranslations'

if (-not $ConfigureCredential -and -not $PoPath) {
    throw 'Pass one or more completed PO files under target/wiki with -PoPath.'
}

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

$credential = Import-Clixml -LiteralPath $credentialPath
if ($credential -isnot [PSCredential]) {
    throw 'The encrypted Wiki import credential is not a PSCredential file.'
}

$poPaths = [Collections.Generic.List[string]]::new()
$separator = [IO.Path]::DirectorySeparatorChar
foreach ($inputPath in $PoPath) {
    $candidate = if ([IO.Path]::IsPathRooted($inputPath)) { $inputPath } else { Join-Path $root $inputPath }
    $fullPath = [IO.Path]::GetFullPath($candidate)
    if (-not $fullPath.StartsWith($workspaceRoot.TrimEnd($separator) + $separator, [StringComparison]::OrdinalIgnoreCase)) {
        throw "PO files must remain under target/wiki: $fullPath"
    }
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        throw "PO file was not found: $fullPath"
    }
    $poPaths.Add($fullPath)
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

function Get-WikiMessages {
    param(
        [string]$Group,
        [string]$Language
    )

    $messages = [Collections.Generic.List[object]]::new()
    $parameters = @{
        action = 'query'
        list = 'messagecollection'
        mcgroup = $Group
        mclanguage = $Language
        mcprop = 'translation'
        mclimit = 'max'
        format = 'json'
    }
    do {
        $response = Invoke-WikiGet $parameters
        if ($response.error) {
            throw "Could not read Translate group '$Group' in '$Language': $($response.error.code) $($response.error.info)"
        }
        foreach ($message in @($response.query.messagecollection)) {
            $messages.Add($message)
        }
        if ($response.continue -and $response.continue.mccontinue) {
            $parameters.mccontinue = [string]$response.continue.mccontinue
        }
        else {
            $parameters.Remove('mccontinue')
            break
        }
    } while ($true)
    return @($messages)
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
    $messageId = ''
    $translation = ''
    $activeField = ''

    foreach ($line in ($Text -split "`r?`n")) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            if ($context) {
                $entries.Add([pscustomobject]@{ Context = $context; MessageId = $messageId; Translation = $translation })
            }
            $context = $null
            $messageId = ''
            $translation = ''
            $activeField = ''
            continue
        }

        if ($line -match '^msgctxt\s+"(.*)"$') {
            $context = ConvertFrom-PoQuotedString -Value $matches[1]
            $activeField = 'msgctxt'
            continue
        }
        if ($line -match '^msgid\s+"(.*)"$') {
            $messageId = ConvertFrom-PoQuotedString -Value $matches[1]
            $activeField = 'msgid'
            continue
        }
        if ($line -match '^msgstr(?:\[\d+\])?\s+"(.*)"$') {
            if ($context) {
                $translation += ConvertFrom-PoQuotedString -Value $matches[1]
            }
            $activeField = 'msgstr'
            continue
        }
        if ($line -match '^msgid_plural\s+') {
            $activeField = 'msgid'
            continue
        }
        if ($line -match '^"(.*)"$' -and $context) {
            $continuation = ConvertFrom-PoQuotedString -Value $matches[1]
            if ($activeField -eq 'msgid') {
                $messageId += $continuation
            }
            elseif ($activeField -eq 'msgstr') {
                $translation += $continuation
            }
        }
    }

    if ($context) {
        $entries.Add([pscustomobject]@{ Context = $context; MessageId = $messageId; Translation = $translation })
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

foreach ($poPath in $poPaths) {
    $poText = Get-Content -LiteralPath $poPath -Raw -Encoding utf8
    $languageMatch = [Regex]::Match($poText, '(?m)"X-Language-Code:\s*([A-Za-z0-9-]+)\\n"')
    if (-not $languageMatch.Success) {
        throw "PO language header is missing or invalid: $poPath"
    }
    $language = $languageMatch.Groups[1].Value.ToLowerInvariant()
    $groupMatch = [Regex]::Match($poText, '(?m)"X-Message-Group:\s*([^"\\]+)\\n"')
    if (-not $groupMatch.Success) {
        throw "PO message group header is missing or invalid: $poPath"
    }
    $groupId = $groupMatch.Groups[1].Value
    if (-not $groupId.StartsWith('page-', [StringComparison]::Ordinal)) {
        throw "PO file does not identify a ShroudForge page group: $groupId"
    }
    $articleTitle = $groupId.Substring('page-'.Length)
    $articleId = Split-Path -Leaf (Split-Path -Parent $poPath)

    $groupList = (Invoke-WikiGet @{
        action = 'query'
        meta = 'messagegroups'
        mgformat = 'flat'
        mgprop = 'id|prioritylangs|sourcelanguage'
        format = 'json'
    }).query.messagegroups
    $messageGroup = $groupList | Where-Object { $_.id -eq $groupId } | Select-Object -First 1
    if (-not $messageGroup) {
        throw "Translate group is not live on the Wiki: $groupId"
    }
    if ($messageGroup.sourcelanguage -ne 'en-gb') {
        throw "Translate group '$groupId' has unexpected source language '$($messageGroup.sourcelanguage)'."
    }
    $priorityLanguages = @($messageGroup.prioritylangs | ForEach-Object { ([string]$_).ToLowerInvariant() })
    if ($language -notin $priorityLanguages) {
        throw "PO language '$language' is not a priority language for '$groupId'."
    }

    $entries = @(Get-PoEntries -Text $poText)
    if ($entries.Count -eq 0) {
        throw "PO file has no msgctxt entries: $poPath"
    }
    $duplicateContexts = @($entries | Group-Object Context | Where-Object Count -gt 1)
    if ($duplicateContexts.Count -gt 0) {
        throw "PO file has duplicate message contexts: $(($duplicateContexts.Name) -join ', ')"
    }
    $sourceMessages = @(Get-WikiMessages -Group $groupId -Language 'en-gb')
    $sourceByKey = @{}
    foreach ($message in $sourceMessages) {
        $sourceByKey[[string]$message.key] = $message
    }
    $unknownContexts = @($entries | Where-Object { -not $sourceByKey.ContainsKey([string]$_.Context) })
    if ($unknownContexts.Count -gt 0) {
        throw "PO contexts are not in live group '$groupId': $(($unknownContexts.Context) -join ', ')"
    }
    $missingContexts = @($sourceMessages | Where-Object { $_.key -notin @($entries.Context) })
    if ($missingContexts.Count -gt 0) {
        throw "PO file omits live message contexts for '$groupId': $(($missingContexts.key) -join ', ')"
    }
    $sourceMismatches = @($entries | Where-Object {
        [string]$_.MessageId -cne [string]$sourceByKey[[string]$_.Context].translation
    })
    if ($sourceMismatches.Count -gt 0) {
        throw "PO source strings do not match the live English units for '$groupId': $(($sourceMismatches.Context) -join ', ')"
    }

    $translatedEntries = @($entries | Where-Object { -not [string]::IsNullOrEmpty($_.Translation) })
    if ($translatedEntries.Count -eq 0) {
        $report.Add([pscustomobject]@{
            ArticleId = $articleId
            Group = $groupId
            Language = $language
            Entries = $entries.Count
            Status = 'Needs translation'
        })
        continue
    }
    if ($translatedEntries.Count -ne $entries.Count) {
        throw "PO file is incomplete for '$groupId' / '$language'. Import it after every unit has a translation: $poPath"
    }

    $liveMessages = @(Get-WikiMessages -Group $groupId -Language $language)
    $liveByKey = @{}
    foreach ($message in $liveMessages) {
        $liveByKey[[string]$message.key] = $message
    }
    $unknownTargetContexts = @($entries | Where-Object { -not $liveByKey.ContainsKey([string]$_.Context) })
    if ($unknownTargetContexts.Count -gt 0) {
        throw "PO contexts are not in live target group '$groupId' / '$language': $(($unknownTargetContexts.Context) -join ', ')"
    }

    if (-not $PSCmdlet.ShouldProcess($poPath, "Upload and import $language PO for $groupId")) {
        $report.Add([pscustomobject]@{
            ArticleId = $articleId
            Group = $groupId
            Language = $language
            Status = 'WhatIf'
            Entries = $entries.Count
        })
        continue
    }

    $page = Invoke-WebRequest -Uri $importUrl -WebSession $session
    if ($page.Content -notmatch 'id=["'']mw-translate-import["'']') {
        throw "The normal Wiki account did not receive the Special:ImportTranslations upload form for '$groupId'."
    }

    $preview = Invoke-WebRequest -Uri $importUrl -Method Post -WebSession $session -Form @{
        title = 'Special:ImportTranslations'
        token = $csrfToken
        'upload-local' = Get-Item -LiteralPath $poPath
    }
    $html = $preview.Content
    Assert-NoImportError -Html $html

    if (-not (Get-InputValue -Html $html -Name 'process')) {
        $plain = [Net.WebUtility]::HtmlDecode(($html -replace '(?is)<script\b.*?</script>|<style\b.*?</style>|<[^>]+>', ' '))
        $plain = [Regex]::Replace($plain, '\s+', ' ').Trim()
        throw "Special:ImportTranslations did not show an import preview for '$groupId' / '$language': $plain"
    }

    $submittedLanguage = Get-InputValue -Html $html -Name 'language'
    if ($submittedLanguage -and $submittedLanguage.ToLowerInvariant() -ne $language) {
        throw "Translate import preview selected '$submittedLanguage' instead of '$language' for '$groupId'."
    }

    $round = 0
    while ($true) {
        $processFlag = Get-InputValue -Html $html -Name 'process'
        if (-not $processFlag) {
            break
        }
        if ($processFlag -ne '1') {
            throw "Unexpected process value on Special:ImportTranslations for '$groupId' / '$language'."
        }
        $round++
        if ($round -gt 20) {
            throw "Translate import for '$groupId' / '$language' did not finish after 20 processing batches."
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

    $verifyMessages = @(Get-WikiMessages -Group $groupId -Language $language)
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
        throw "Import finished but $($notImported.Count) PO entries do not match saved translations in '$groupId' / '$language': $(($notImported.Context) -join ', ')"
    }

    $report.Add([pscustomobject]@{
        ArticleId = $articleId
        Group = $groupId
        Language = $language
        Entries = $entries.Count
        ImportedEntries = $translatedEntries.Count
        Status = 'Imported through Special:ImportTranslations and verified'
    })
    [void]$expectedTranslationPages.Add("$articleTitle/$language")
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
