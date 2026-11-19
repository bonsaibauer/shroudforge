[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
param(
    [Parameter(Mandatory)]
    [ValidatePattern('^[a-z0-9]+(?:-[a-z0-9]+)*$')]
    [string]$ArticleId,

    [Parameter(Mandatory)]
    [ValidateNotNullOrEmpty()]
    [string]$Title,

    [Parameter(Mandatory)]
    [ValidateNotNullOrEmpty()]
    [string]$Description,

    [string]$PictureFileName
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$workspaceRoot = [IO.Path]::GetFullPath((Join-Path $root 'target\wiki'))
$articleDirectory = [IO.Path]::GetFullPath((Join-Path $workspaceRoot $ArticleId))
$separator = [IO.Path]::DirectorySeparatorChar
if (-not $articleDirectory.StartsWith($workspaceRoot.TrimEnd($separator) + $separator, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'The article workspace must remain below target/wiki.'
}
if (Test-Path -LiteralPath $articleDirectory) {
    throw "The temporary workspace already exists and was left unchanged: $articleDirectory"
}
if ($Title.Contains('--') -or $Description.Contains('--')) {
    throw 'Title and description cannot contain -- because they are written inside an HTML comment.'
}
if (-not $PictureFileName) {
    $PictureFileName = "ShroudForge-Wiki-$ArticleId.svg"
}
if ($PictureFileName -notmatch '^[A-Za-z0-9._ -]+\.(svg|png|gif|jpe?g|webp)$') {
    throw 'PictureFileName must be a simple MediaWiki filename with a supported image extension.'
}

$wikitextPath = Join-Path $articleDirectory 'en.wikitext'
$wikitext = @"
<languages />

<!-- ShroudForge Modloader Wiki Post
title: $Title
description: $Description
language: en-GB
picture:$PictureFileName
creator: Bonsaibauer
-->

[[File:$PictureFileName|thumb|left|600px|$Title]]
<br style="clear:both;" />

<translate>

$Description

== Overview ==

Replace this paragraph with the English article text.
</translate>

[[Category:guide:Shroudforge]]
"@

if ($PSCmdlet.ShouldProcess($articleDirectory, 'Create temporary online Wiki article workspace')) {
    New-Item -ItemType Directory -Path $articleDirectory | Out-Null
    [IO.File]::WriteAllText($wikitextPath, $wikitext, [Text.UTF8Encoding]::new($false))
    Write-Host "Created temporary English source: $wikitextPath"
    Write-Host "Place the named image in this directory before publishing: $PictureFileName"
}
