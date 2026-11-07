from __future__ import annotations

import csv
import html
import re
import shutil
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "modules" / "wiki" / "articles"
OUTPUT = ROOT / "docs" / "mediawiki-migration"
CATEGORY = "guide:shroudforge"
CREATOR = "Bonsaibauer"
REVISION_TIMESTAMP = "2026-11-05T12:00:00Z"
XML_NAMESPACE = "http://www.mediawiki.org/xml/export-0.11/"
XML = f"{{{XML_NAMESPACE}}}"

LANGUAGE_NAMES = {
    "de": "Deutsch",
    "en": "English",
    "es": "Español",
    "fr": "Français",
}


@dataclass(frozen=True)
class Article:
    article_id: str
    locale: str
    title: str
    description: str
    body: str
    source_path: Path
    asset_name: str

    @property
    def page_title(self) -> str:
        return f"{self.title} ({self.locale})"


def source_parts(path: Path) -> tuple[str, str, str]:
    text = path.read_text(encoding="utf-8").replace("\r\n", "\n").replace("\r", "\n")
    heading = re.match(r"^#\s+(.+?)\s*\n+", text)
    if not heading:
        raise ValueError(f"Article has no first-level title: {path}")
    title = heading.group(1).strip()
    content = text[heading.end():]
    content = content.lstrip("\n")
    description_match = re.match(r"(.*?)(?:\n[ \t]*\n)(.*)\Z", content, flags=re.DOTALL)
    if not description_match:
        raise ValueError(f"Article has no description: {path}")
    description = re.sub(r"\s*\n\s*", " ", description_match.group(1)).strip()
    body = description_match.group(2)
    body = re.sub(r"<!--\s*wiki-category:\s*[^>]*?-->[ \t]*", "", body, flags=re.IGNORECASE)
    body = re.sub(r"^\[!\[[^\]]*\]\([^)]*\)\]\([^)]*\)[ \t]*(?:\n[ \t]*)*", "", body)
    body = body.strip("\n")
    body = re.sub(r"\n[ \t]*\n(?:[ \t]*\n)+", "\n\n", body).strip()
    return title, description, body


def article_inventory() -> list[Article]:
    source_files = sorted(SOURCE.glob("*/*.md"), key=lambda path: (path.parent.name, path.stem))
    articles: list[Article] = []
    for path in source_files:
        title, description, body = source_parts(path)
        article_id = path.parent.name
        asset_name = f"ShroudForge-Wiki-{article_id}.svg"
        articles.append(Article(article_id, path.stem, title, description, body, path, asset_name))

    if len({(article.article_id, article.locale) for article in articles}) != len(articles):
        raise ValueError("Duplicate article ID and locale pair")
    return articles


def page_title_map(articles: list[Article]) -> dict[tuple[str, str], str]:
    return {(article.article_id, article.locale): article.page_title for article in articles}


def inline_wikitext(text: str, article: Article, titles: dict[tuple[str, str], str]) -> str:
    def internal_link(match: re.Match[str]) -> str:
        label, target = match.group(1), match.group(2)
        if target.startswith("#wiki/"):
            target_id = target.removeprefix("#wiki/")
            page = titles.get((target_id, article.locale)) or titles.get((target_id, "en"))
            if page is None:
                raise ValueError(f"Unresolved internal Wiki link in {article.source_path}: {target}")
            return f"[[{page}|{inline_wikitext(label, article, titles)}]]"
        if target.startswith("#"):
            return f"[[#{target[1:]}|{inline_wikitext(label, article, titles)}]]"
        return f"[{target} {inline_wikitext(label, article, titles)}]"

    text = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", internal_link, text)

    def code(match: re.Match[str]) -> str:
        value = html.escape(match.group(1), quote=False)
        for character, entity in (("{", "&#123;"), ("}", "&#125;"), ("|", "&#124;"), ("[", "&#91;"), ("]", "&#93;")):
            value = value.replace(character, entity)
        return f"<code>{value}</code>"

    text = re.sub(r"`([^`]+)`", code, text)
    text = re.sub(r"\*\*(.+?)\*\*", r"'''\1'''", text)
    text = re.sub(r"__(.+?)__", r"'''\1'''", text)
    text = re.sub(r"(?<!\*)\*([^*\n]+?)\*(?!\*)", r"''\1''", text)
    return text


def convert_table(lines: list[str], article: Article, titles: dict[tuple[str, str], str]) -> str:
    def cells(line: str) -> list[str]:
        value = line.strip()
        if value.startswith("|"):
            value = value[1:]
        if value.endswith("|"):
            value = value[:-1]
        return [cell.strip() for cell in value.split("|")]

    rows = [cells(line) for line in lines]
    output = ['{| class="wikitable"']
    if len(rows) >= 2 and all(re.fullmatch(r"\s*:?-{3,}:?\s*", cell) for cell in rows[1]):
        output.append("! " + " !! ".join(inline_wikitext(cell, article, titles) for cell in rows[0]))
        rows = rows[2:]
    for row in rows:
        output.append("|-")
        output.append("| " + " || ".join(inline_wikitext(cell, article, titles) for cell in row))
    output.append("|}")
    return "\n".join(output)


def convert_body(body: str, article: Article, titles: dict[tuple[str, str], str]) -> str:
    lines = body.splitlines()
    output: list[str] = []
    index = 0
    list_parent = ""
    while index < len(lines):
        line = lines[index]
        stripped = line.strip()
        if not stripped:
            output.append("")
            list_parent = ""
            index += 1
            continue

        if stripped.startswith("```") or stripped.startswith("~~~"):
            fence = stripped[:3]
            index += 1
            code_lines: list[str] = []
            while index < len(lines) and not lines[index].strip().startswith(fence):
                code_lines.append(lines[index])
                index += 1
            if index < len(lines):
                index += 1
            output.extend(["<pre>", html.escape("\n".join(code_lines), quote=False), "</pre>"])
            list_parent = ""
            continue

        if stripped.startswith("|") and index + 1 < len(lines) and re.fullmatch(r"\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*", lines[index + 1]):
            table_lines: list[str] = []
            while index < len(lines) and lines[index].strip().startswith("|"):
                table_lines.append(lines[index])
                index += 1
            output.append(convert_table(table_lines, article, titles))
            output.append("")
            list_parent = ""
            continue

        heading = re.match(r"^(#{2,6})\s+(.+?)\s*#*\s*$", stripped)
        if heading:
            level = len(heading.group(1))
            equals = "=" * level
            output.append(f"{equals} {inline_wikitext(heading.group(2), article, titles)} {equals}")
            output.append("")
            list_parent = ""
            index += 1
            continue

        list_item = re.match(r"^(\s*)([-+*]|\d+[.)])\s+(.+)$", line)
        if list_item:
            indent = len(list_item.group(1).replace("\t", "    "))
            marker = "#" if list_item.group(2)[0].isdigit() else "*"
            if indent == 0:
                list_parent = marker
                prefix = marker
            else:
                prefix = (list_parent or marker) + marker
            output.append(prefix + " " + inline_wikitext(list_item.group(3), article, titles))
            index += 1
            continue

        if stripped.startswith("> "):
            output.append("<blockquote>" + inline_wikitext(stripped[2:], article, titles) + "</blockquote>")
            list_parent = ""
            index += 1
            continue

        output.append(inline_wikitext(stripped, article, titles))
        list_parent = ""
        index += 1

    converted = "\n".join(output)
    converted = re.sub(r"\n{3,}", "\n\n", converted).strip()
    return converted


def page_wikitext(article: Article, titles: dict[tuple[str, str], str]) -> str:
    source_relative = article.source_path.relative_to(ROOT).as_posix()
    metadata = "\n".join((
        "<!-- ShroudForge MediaWiki migration metadata",
        f"title: {article.title}",
        f"description: {article.description}",
        f"language: {article.locale}",
        f"creator: {CREATOR}",
        f"source: {source_relative}",
        "-->",
    ))
    illustration = (
        f"[[File:{article.asset_name}|thumb|right|alt={article.title}|{article.title}]]"
    )
    body = convert_body(article.body, article, titles)
    parts = [metadata, inline_wikitext(article.description, article, titles), illustration]
    if body:
        parts.append(body)
    parts.append(f"[[Category:{CATEGORY}]]")
    return "\n\n".join(parts).strip() + "\n"


def add_text(parent: ET.Element, name: str, value: str) -> ET.Element:
    child = ET.SubElement(parent, XML + name)
    child.text = value
    return child


def export_xml(articles: list[Article], titles: dict[tuple[str, str], str]) -> None:
    ET.register_namespace("", XML_NAMESPACE)
    root = ET.Element(XML + "mediawiki", {
        "version": "0.11",
        "xml:lang": "en",
    })
    siteinfo = ET.SubElement(root, XML + "siteinfo")
    add_text(siteinfo, "sitename", "ShroudForge Modloader Wiki migration")
    add_text(siteinfo, "dbname", "shroudforge_modloader_wiki")
    add_text(siteinfo, "base", "https://github.com/bonsaibauer/shroudforge")
    add_text(siteinfo, "generator", "ShroudForge MediaWiki migration package")
    add_text(siteinfo, "case", "first-letter")
    namespaces = ET.SubElement(siteinfo, XML + "namespaces")
    namespace = ET.SubElement(namespaces, XML + "namespace", {"key": "0", "case": "first-letter"})
    namespace.text = ""
    namespace = ET.SubElement(namespaces, XML + "namespace", {"key": "14", "case": "first-letter"})
    namespace.text = "Category"

    pages: list[tuple[str, int, str]] = []
    for article in articles:
        pages.append((article.page_title, 0, page_wikitext(article, titles)))
    category_text = (
        "Guides and help articles from the ShroudForge Modloader Wiki.\n\n"
        f"This category was requested as <code>{CATEGORY}</code>.\n"
    )
    pages.append((f"Category:{CATEGORY}", 14, category_text))

    for page_id, (title, namespace_id, content) in enumerate(pages, start=1):
        page = ET.SubElement(root, XML + "page")
        add_text(page, "title", title)
        add_text(page, "ns", str(namespace_id))
        add_text(page, "id", str(page_id))
        revision = ET.SubElement(page, XML + "revision")
        add_text(revision, "id", str(1000 + page_id))
        add_text(revision, "timestamp", REVISION_TIMESTAMP)
        contributor = ET.SubElement(revision, XML + "contributor")
        add_text(contributor, "username", CREATOR)
        add_text(revision, "comment", "Initial import of a ShroudForge Modloader Wiki article")
        add_text(revision, "model", "wikitext")
        add_text(revision, "format", "text/x-wiki")
        text = ET.SubElement(revision, XML + "text", {"{http://www.w3.org/XML/1998/namespace}space": "preserve"})
        text.text = content

    xml_path = OUTPUT / "import" / "shroudforge-modloader-wiki.xml"
    xml_path.parent.mkdir(parents=True, exist_ok=True)
    ET.indent(root, space="  ")
    ET.ElementTree(root).write(xml_path, encoding="utf-8", xml_declaration=True)


def main() -> None:
    articles = article_inventory()
    titles = page_title_map(articles)
    source_ids = sorted({article.article_id for article in articles})

    (OUTPUT / "articles").mkdir(parents=True, exist_ok=True)
    assets_dir = OUTPUT / "assets"
    assets_dir.mkdir(parents=True, exist_ok=True)

    for article_id in source_ids:
        (OUTPUT / "articles" / article_id).mkdir(parents=True, exist_ok=True)
    for article in articles:
        target = OUTPUT / "articles" / article.article_id / f"{article.locale}.wikitext"
        target.write_text(page_wikitext(article, titles), encoding="utf-8", newline="\n")

    asset_rows: list[dict[str, str]] = []
    for article_id in source_ids:
        source_asset = SOURCE / article_id / "illustration.svg"
        asset_name = f"ShroudForge-Wiki-{article_id}.svg"
        if not source_asset.is_file():
            raise FileNotFoundError(f"Missing illustration for {article_id}: {source_asset}")
        shutil.copyfile(source_asset, assets_dir / asset_name)
        locales = sorted(article.locale for article in articles if article.article_id == article_id)
        titles_text = " | ".join(
            f"{LANGUAGE_NAMES.get(locale, locale)}: {titles[(article_id, locale)]}"
            for locale in locales
        )
        asset_rows.append({
            "file_title": asset_name,
            "article_id": article_id,
            "source_asset": source_asset.relative_to(ROOT).as_posix(),
            "import_asset": f"assets/{asset_name}",
            "page_titles": titles_text,
            "locales": ",".join(locales),
        })

    with (assets_dir / "asset-map.csv").open("w", encoding="utf-8-sig", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=list(asset_rows[0]))
        writer.writeheader()
        writer.writerows(asset_rows)

    with (OUTPUT / "pages.csv").open("w", encoding="utf-8-sig", newline="") as output:
        writer = csv.writer(output)
        writer.writerow(["page_title", "article_title", "description", "locale", "creator", "category", "source", "asset_file"])
        for article in articles:
            writer.writerow([
                article.page_title,
                article.title,
                article.description,
                article.locale,
                CREATOR,
                CATEGORY,
                article.source_path.relative_to(ROOT).as_posix(),
                article.asset_name,
            ])

    export_xml(articles, titles)
    print(f"Created {len(articles)} pages from {len(source_ids)} article IDs and {len(source_ids)} SVG assets.")


if __name__ == "__main__":
    main()
