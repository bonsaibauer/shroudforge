---
name: create-translated-mediawiki-article
description: Create or migrate ShroudForge MediaWiki articles with Translate, prepare locally written PO translations, set supported priority languages, import them through the wiki form, and verify the rendered language pages.
---

# Create and Translate ShroudForge MediaWiki Articles

Work from the repository root and preserve existing changes. Use this skill for a new MediaWiki article or a migration from the offline Modloader Wiki. For a batch, keep one English source page and one Translate message group per article. The detailed, maintained procedure and commands are in [`docs/mediawiki-translate/README.md`](../../../../docs/mediawiki-translate/README.md).

## Prepare the article

Inspect the English and localized files under `modules/wiki/articles/<id>/`. Reuse the article's existing title, text, illustration, and available translations. When migrating, compare the English source units with each local translation. Resolve missing, extra, or reordered units before preparing POs. Keep technical names, commands, paths, links, and code intact.

Use the ShroudForge page schema. Metadata, illustration, and categories stay outside `<translate>`. The English summary and article text stay inside it. Let Translate generate translation-unit markers and page subpages.

```wikitext
<languages />

<!-- ShroudForge Modloader Wiki Post
title: <English title>
description: <English summary>
language: en-GB
picture:<MediaWiki image filename>
creator: Bonsaibauer
-->

[[File:<MediaWiki image filename>|thumb|left|600px|<English title>]]
<br style="clear:both;" />

<translate>

<English summary>

== <Section heading> ==

<English article text>
</translate>

[[Category:guide:Shroudforge]]
```

Keep the metadata `picture:` value identical to the `File:` name. Use `[[Category:guide:Shroudforge]]` first. If an article needs a subject category, append it after the guide category. `Startup stops because asset backups are incompatible` also uses `[[Category:Problems:Shroudforge]]`. Shared metadata, images, and categories must not be placed inside the translated block.

## Write local PO translations

Translate the source text locally. Do not connect or call an external translation service unless the user explicitly asks for one. Use existing local article translations where they exist. For the other supported languages, add the translated units to `docs/mediawiki-translate/offline-translations.json` and generate the POs with the existing scripts.

PO files live under `docs/mediawiki-translate/<article-id>/`. Preserve the live group's `X-Message-Group`, `X-Language-Code`, `msgctxt`, source strings, and every translation unit, including the translatable page title. Use the live Translate message collection as the authority for contexts. Check that each translation is paired with the intended English unit, not only that the entry counts match. Never invent contexts or silently drop a unit.

The current accepted target locales are `de-at`, `de-ch`, `de`, `es`, `fr`, `hi`, `it`, `ja`, `nl`, `pl`, `sv`, and `zh`. `en-GB` is the source language. The wiki rejects `en-US` as a priority language. Read `PRIORITY_LANGUAGES` in `docs/mediawiki-translate/build-pages.py` before adding or changing locales. German variants are generated from `de`, with `ß` converted to `ss` for `de-ch`.

## Publish, mark, and import

Use the existing workflow rather than building a second importer:

```powershell
pwsh -NoProfile -File docs/mediawiki-translate/run.ps1 -WhatIf
pwsh -NoProfile -File docs/mediawiki-translate/run.ps1
```

For one article, add `-ArticleId <article-id>`. The workflow prepares localized POs, fills local translations, builds English Wikitext and the manifest, publishes missing source pages and images, marks the source for translation with title translation enabled, applies the supported priority languages, then imports the POs through [Special:ImportTranslations](https://wiki.shroudforge.net/index.php/Special:ImportTranslations). `-WhatIf` avoids live wiki mutations but still regenerates local PO files and the manifest.

`publish.ps1` uses the locally stored bot API credential for page creation, file upload, and `pagetranslation`. It leaves existing article text unchanged. `import-po.ps1` uses the separately stored normal Wiki account with `translate-import`. A BotPassword cannot use the Special: page. Configure the normal account with:

```powershell
pwsh -NoProfile -File docs/mediawiki-translate/import-po.ps1 -ConfigureCredential
```

Credentials stay encrypted in the Windows user profile. Never print or commit them, and never ask the user to paste a password into chat. The PO import is a Special:ImportTranslations form workflow, not an Action API or XML import. Rights such as `import` and `importupload` are unrelated.

## Verify rendered pages

Verify the source group and `en-gb` source language, all expected priority languages, and every imported PO entry. Then check the actual language subpages such as `<English title>/de` and `<English title>/ja`. A successful PO import means Translate stored the message translations. It does not guarantee that the visible language subpages have rendered yet.

Translate updates those pages with `TranslateRenderJob` through MediaWiki's server-side job queue. If the imported messages are present but language subpages are missing, do not create static `/de` or other language pages by hand and do not claim the task is complete. The Wiki API cannot run the server maintenance queue. A host administrator must inspect the job types with `php maintenance/run.php showJobs --group` and process the Translate render jobs with `maintenance/run.php runJobs`. See the job-queue section in the repository README for a bounded example.

For offline Modloader article changes, follow [add-modloader-wiki-article](../add-modloader-wiki-article/SKILL.md). Do not confuse the older `docs/mediawiki-migration/` package with the current MediaWiki Translate workflow.
