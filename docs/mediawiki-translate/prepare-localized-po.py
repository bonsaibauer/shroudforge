from __future__ import annotations

import ast
import importlib.util
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "docs" / "mediawiki-translate"
MIGRATION_BUILDER = ROOT / "docs" / "mediawiki-migration" / "build-package.py"
SOURCE_LANGUAGES = ("es", "fr")
GERMAN_VARIANTS = ("de-at", "de-ch")


def load_builder():
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location("shroudforge_mediawiki_builder", MIGRATION_BUILDER)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Cannot load the article converter: {MIGRATION_BUILDER}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def parse_po_string(value: str) -> str:
    try:
        return ast.literal_eval(value)
    except (SyntaxError, ValueError) as error:
        raise ValueError("Unsupported PO string escape") from error


def parse_po_block(block: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    active = ""
    for line in block.splitlines():
        match = re.fullmatch(r'(msgctxt|msgid|msgstr)\s+(".*")', line)
        if match:
            active = match.group(1)
            fields[active] = parse_po_string(match.group(2))
            continue
        match = re.fullmatch(r'(".*")', line)
        if match and active:
            fields[active] += parse_po_string(match.group(1))
        else:
            active = ""
    return fields


def quoted(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def replace_msgstr(block: str, translation: str) -> str:
    result: list[str] = []
    active = ""
    replaced = False
    for line in block.splitlines():
        if re.match(r"^msgstr(?:\[\d+\])?\s+", line):
            result.append("msgstr " + quoted(translation))
            active = "msgstr"
            replaced = True
            continue
        if line.startswith('"') and active == "msgstr":
            continue
        if re.match(r"^(?:msgctxt|msgid|msgid_plural|msgstr)(?:\[\d+\])?\s+", line):
            active = ""
        result.append(line)
    if not replaced:
        raise ValueError("PO message has no msgstr field")
    return "\n".join(result)


def set_language_header(text: str, code: str, label: str) -> str:
    text = text.replace("# Exported from Shroudforge", "# Exported from Shroudforge")
    text = re.sub(r"(?m)^# Translation of (.+?) to .+$", rf"# Translation of \1 to {label}", text, count=1)
    text = text.replace('"Language: de\\n"', f'"Language: {code.replace("-", "_")}\\n"')
    text = text.replace('"X-Language-Code: de\\n"', f'"X-Language-Code: {code}\\n"')
    if '"X-Language-Code: ' + code + r'\n"' not in text:
        raise ValueError(f"Could not update PO language header to '{code}'")
    return text


def render_po(base_text: str, translations: dict[str, str], code: str, label: str) -> str:
    blocks = re.split(r"\n\s*\n", base_text.rstrip())
    rendered: list[str] = []
    seen: set[str] = set()
    for block in blocks:
        fields = parse_po_block(block)
        if "msgctxt" not in fields:
            rendered.append(set_language_header(block, code, label))
            continue
        context = fields["msgctxt"]
        if context not in translations:
            raise ValueError(f"No translation provided for PO context '{context}'")
        translation = translations[context]
        if code == "de-ch":
            translation = translation.replace("ß", "ss")
        rendered.append(replace_msgstr(block, translation))
        seen.add(context)
    missing = set(translations) - seen
    if missing:
        raise ValueError("Translations do not match PO contexts: " + ", ".join(sorted(missing)))
    return "\n\n".join(rendered) + "\n"


def page_units(article, builder, titles) -> tuple[list[str], str]:
    description = builder.inline_wikitext(article.description, article, titles)
    body = builder.convert_body(article.body, article, titles)
    content = "\n\n".join(part for part in (description, body) if part)
    units = [part.strip() for part in re.split(r"\n\s*\n", content) if part.strip()]
    return units, article.title


def main() -> None:
    builder = load_builder()
    articles = builder.article_inventory()
    titles = builder.page_title_map(articles)
    by_id_language = {(article.article_id, article.locale): article for article in articles}
    generated = 0
    skipped: list[str] = []

    for source_article in (article for article in articles if article.locale == "en"):
        article_dir = OUTPUT / source_article.article_id
        de_path = article_dir / "de.po"
        if not de_path.is_file():
            skipped.append(f"{source_article.article_id}: missing de.po")
            continue

        base_text = de_path.read_text(encoding="utf-8")
        base_blocks = re.split(r"\n\s*\n", base_text.rstrip())
        base_entries = [parse_po_block(block) for block in base_blocks if "msgctxt " in block]
        de_translations = {entry["msgctxt"]: entry["msgstr"] for entry in base_entries}
        source_units, _ = page_units(source_article, builder, titles)
        title_entry = base_entries[0]
        if not title_entry["msgctxt"].endswith("/Page_display_title"):
            raise ValueError(f"{source_article.article_id}: the first PO entry is not the page title")
        aligned = len(base_entries) == len(source_units) + 1
        mismatched_contexts: list[str] = []
        if aligned:
            for entry, source_unit in zip(base_entries[1:], source_units, strict=True):
                if entry["msgid"] != source_unit:
                    aligned = False
                    mismatched_contexts.append(entry["msgctxt"])

        for code in GERMAN_VARIANTS:
            translations = dict(de_translations)
            target_path = article_dir / f"{code}.po"
            target_path.write_text(
                render_po(base_text, translations, code, "German"),
                encoding="utf-8",
                newline="\n",
            )
            generated += 1

        for language in SOURCE_LANGUAGES:
            target_article = by_id_language.get((source_article.article_id, language))
            if target_article is None:
                skipped.append(f"{source_article.article_id}: no {language} article source")
                continue
            if not aligned:
                contexts = ", ".join(mismatched_contexts) or "message count mismatch"
                skipped.append(f"{source_article.article_id}: {language} source units do not match ({contexts})")
                continue
            target_units, translated_title = page_units(target_article, builder, titles)
            if len(target_units) != len(source_units):
                raise ValueError(
                    f"{source_article.article_id}/{language}: source has {len(source_units)} units, "
                    f"translation has {len(target_units)}"
                )
            translations = {title_entry["msgctxt"]: translated_title}
            translations.update(
                {
                    entry["msgctxt"]: translated
                    for entry, source_unit, translated in zip(
                        base_entries[1:], source_units, target_units, strict=True
                    )
                    if entry["msgid"] == source_unit
                }
            )
            if len(translations) != len(base_entries):
                raise ValueError(f"{source_article.article_id}/{language}: translation units did not align")
            target_path = article_dir / f"{language}.po"
            target_path.write_text(
                render_po(base_text, translations, language, "Spanish" if language == "es" else "French"),
                encoding="utf-8",
                newline="\n",
            )
            generated += 1

    print(f"Generated {generated} localized PO files.")
    for item in skipped:
        print(f"Skipped {item}")


if __name__ == "__main__":
    main()
