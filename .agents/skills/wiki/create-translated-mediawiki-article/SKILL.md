---
name: create-translated-mediawiki-article
description: Publish English ShroudForge MediaWiki articles through the live API, mark them for Translate, and optionally create and import PO translations separately.
---

# Publish ShroudForge MediaWiki Articles Online

Create public MediaWiki articles directly through `https://wiki.shroudforge.net/api.php`. Keep permanent procedures and scripts in `dev/wiki/`. Keep article drafts, media, and PO files only in the ignored temporary workspace `target/wiki/<article-id>/`. Do not create an online Wiki article under `modules/wiki/articles/`, `docs/`, or another tracked content directory. `modules/wiki/articles/` belongs to the Modloader's separate offline help system.

Read [`dev/wiki/README.md`](../../../../dev/wiki/README.md) for the maintained commands and two publication methods.

## Method 1: English source page

1. Create a temporary workspace with `dev/wiki/new_article_workspace.ps1` or make `target/wiki/<article-id>/` manually.
2. Write the English page to `en.wikitext` and place referenced image files in that workspace. Use the ShroudForge page schema. Metadata, illustration, and categories stay outside `<translate>`. The English summary and article text stay inside it.
3. Run `dev/wiki/publish_article.ps1 -WhatIf` to review the page and MediaWiki actions.
4. Run it without `-WhatIf` to upload missing images and the English source through the Action API, mark the page for translation, enable title translation, and set the accepted priority languages.

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

Keep the metadata `picture:` value identical to the `File:` name. Use `[[Category:guide:Shroudforge]]` first. Add subject categories after it when needed. The source language is `en-GB`. Supported priority languages are `de-at`, `de-ch`, `de`, `es`, `fr`, `hi`, `it`, `ja`, `nl`, `pl`, `sv`, and `zh`. The Wiki rejects `en-US` as a priority language.

Existing English pages are left unchanged by default. Pass `-UpdateExisting` only when the user explicitly asks to replace the live source text. The script uses `baserevid` and then marks the resulting revision.

## Method 2: PO translations in a separate upload

After the source page is marked for translation, use `dev/wiki/create_po_templates.ps1` with the live group ID `page-<English title>`. It reads current message contexts and English source strings from the Wiki API and writes blank PO templates under the temporary article workspace. Never invent `msgctxt` values or rely on a local article export as the context authority.

Fill every `msgstr` while preserving the group and language headers, contexts, source strings, and all units including the page title. Import each completed file separately with `dev/wiki/import_po.ps1`. The importer checks that the PO covers the current live group, verifies each source unit, submits it through `Special:ImportTranslations`, and confirms the saved translations.

PO import requires the normal Wiki account with `translate-import`. A BotPassword cannot access the Special page. Store API and import credentials with `dev/wiki/configure_credentials.ps1`. Credentials remain encrypted under the Windows user profile. Never print or commit credentials, and never ask the user to paste a password into chat.

Translate creates language subpages through `TranslateRenderJob`. If all PO messages are saved but a language subpage is missing, report the queued-job condition. Do not create static language pages by hand. The Wiki host must process the Translate render jobs.

## Media and offline help

The current Wiki upload allowlist accepts PNG, GIF, JPG, JPEG, WebP, and SVG. PDF upload is rejected. Link the original PDF or place rendered page images in the ignored temporary workspace.

For offline Modloader help articles, use [`add-modloader-wiki-article`](../add-modloader-wiki-article/SKILL.md). That workflow is separate from public online MediaWiki pages.
