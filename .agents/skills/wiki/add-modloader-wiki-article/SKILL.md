---
name: add-modloader-wiki-article
description: Add an offline help article and optional contextual help banner to the Modloader Wiki, using modules/wiki/articles/ and its existing article reader. Use for installed Modloader help content and WikiHelp integration.
---

# Add a Modloader Wiki Article

Work from the repository root. Read `.agents/skills/repository/repository-working-rules.md`, `docs/development/code-ownership.en.md`, and the existing article closest to the requested topic. Preserve existing changes.

## Author the Offline Article

- Create `modules/wiki/articles/<article-id>/<locale>.md` with a stable lowercase hyphenated article ID. Supply the requested translations and preserve existing ones when editing.
- Use a first-level heading for the localized title. Put the localized banner summary in the first paragraph immediately after it. The remaining Markdown is the expanded article body.
- Use the existing article format. This reader does not consume the website's JSON document header.
- Put the shared banner artwork at `modules/wiki/articles/<article-id>/illustration.svg`. Other inline images may be package-local SVG, PNG, JPG, or WebP files. The reader only discovers files directly inside that article folder.
- Use `#wiki/<article-id>` for links to another Wiki article. Local section links use the renderer's heading slugs. External HTTPS and mail links go through the existing host command.

## Use the Existing Reader and UI

Read `modules/wiki/ui/read-wiki-articles.ts` and `show-wiki-article.tsx` before extending the format. Vite discovers article Markdown and images automatically. Locale fallback tries the requested locale, its language, English, German, then an available translation. The list sorts by localized title.

The full article list is `modules/wiki/ui/show-wiki.tsx`. To add requested contextual help to a feature, use `WikiHelp` from `modules/wiki/ui/show-wiki-help.tsx` with the same article ID in that feature's owning UI. Existing examples include `modules/hotkeys/ui/show-hotkeys.tsx` and `modules/modloader/ui/show-compatibility.tsx`.

Keep one article source for the list and contextual banner. Preserve the existing escaping, URL handling, image path checks, fallback artwork, and UI event logging. Automatic discovery needs no article registry.

Published website pages have a separate owner under `docs/pages/` and a different metadata and build contract. Add those only when the task also requests website content.

## Verify

Build the Modloader frontend with `./dev/build/build_modloader_frontend.ps1`, using its installed-dependency option when appropriate. Check the new article's title, summary, body, illustration, requested translations, and links through the existing Wiki UI when functional verification is requested. Also check its contextual banner if one was added.

Report the article ID, source and asset paths, locales, contextual UI caller, build result, and functional checks actually performed.
