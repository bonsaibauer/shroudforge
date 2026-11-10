from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "docs" / "mediawiki-translate"
MIGRATION_BUILDER = ROOT / "docs" / "mediawiki-migration" / "build-package.py"
CREATOR = "Bonsaibauer"
SOURCE_LANGUAGE = "en-GB"
PRIORITY_LANGUAGES = [
    "de-at",
    "de-ch",
    "de",
    "es",
    "fr",
    "hi",
    "it",
    "ja",
    "nl",
    "pl",
    "sv",
    "zh",
]
PRIMARY_CATEGORY = "guide:Shroudforge"
ARTICLE_CATEGORIES = {
    "mod-settings-search": "Modding:Shroudforge",
}
EXTRA_CATEGORIES = {
    "mod-settings-search": ["Mod-extended-json"],
    "startup-asset-backups": ["Problems:Shroudforge"],
}


def load_migration_builder():
    sys.dont_write_bytecode = True
    name = "shroudforge_mediawiki_migration_builder"
    spec = importlib.util.spec_from_file_location(name, MIGRATION_BUILDER)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Cannot load Markdown-to-Wikitext converter: {MIGRATION_BUILDER}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def article_wikitext(article, builder, titles) -> str:
    primary_category = ARTICLE_CATEGORIES.get(article.article_id, PRIMARY_CATEGORY)
    metadata = "\n".join(
        (
            "<!-- ShroudForge Modloader Wiki Post",
            f"title: {article.title}",
            f"description: {article.description}",
            f"language: {SOURCE_LANGUAGE}",
            f"picture:{article.asset_name}",
            f"creator: {CREATOR}",
            "-->",
        )
    )
    image = f"[[File:{article.asset_name}|thumb|left|600px|{article.title}]]"
    translatable = [builder.inline_wikitext(article.description, article, titles)]
    body = builder.convert_body(article.body, article, titles)
    if body:
        translatable.append(body)

    parts = [
        "<languages />",
        metadata,
        image,
        '<br style="clear:both;" />',
        "<translate>\n\n" + "\n\n".join(translatable) + "\n</translate>",
        f"[[Category:{primary_category}]]",
    ]
    parts.extend(f"[[Category:{category}]]" for category in EXTRA_CATEGORIES.get(article.article_id, []))
    return "\n\n".join(parts) + "\n"


def main() -> None:
    builder = load_migration_builder()
    articles = builder.article_inventory()
    titles = builder.page_title_map(articles)
    manifest = []

    for article in articles:
        if article.locale != "en":
            continue

        page_path = OUTPUT / article.article_id / "en.wikitext"
        page_path.parent.mkdir(parents=True, exist_ok=True)
        page_path.write_text(article_wikitext(article, builder, titles), encoding="utf-8", newline="\n")

        asset_path = ROOT / "modules" / "wiki" / "articles" / article.article_id / "illustration.svg"
        po_path = OUTPUT / article.article_id / "de.po"
        po_files = sorted(OUTPUT.joinpath(article.article_id).glob("*.po"))
        manifest.append(
            {
                "id": article.article_id,
                "title": article.title,
                "sourceLanguage": SOURCE_LANGUAGE,
                "priorityLanguages": PRIORITY_LANGUAGES,
                "group": f"page-{article.title}",
                "wikitext": page_path.relative_to(ROOT).as_posix(),
                "asset": asset_path.relative_to(ROOT).as_posix(),
                "assetTitle": article.asset_name,
                "po": po_path.relative_to(ROOT).as_posix() if po_path.is_file() else None,
                "poFiles": [path.relative_to(ROOT).as_posix() for path in po_files],
                "categories": [
                    ARTICLE_CATEGORIES.get(article.article_id, PRIMARY_CATEGORY),
                    *EXTRA_CATEGORIES.get(article.article_id, []),
                ],
            }
        )

    manifest_path = OUTPUT / "publish-manifest.json"
    manifest_path.write_text(
        json.dumps({"articles": manifest}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    print(f"Generated {len(manifest)} English Translate pages and the publish manifest.")


if __name__ == "__main__":
    main()
