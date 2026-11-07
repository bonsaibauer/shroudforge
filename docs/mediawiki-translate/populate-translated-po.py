from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "docs" / "mediawiki-translate"
TRANSLATIONS_PATH = OUTPUT / "offline-translations.json"
PO_PREPARER = OUTPUT / "prepare-localized-po.py"
LANGUAGE_LABELS = {
    "es": "Spanish",
    "fr": "French",
    "hi": "Hindi",
    "it": "Italian",
    "ja": "Japanese",
    "nl": "Dutch",
    "pl": "Polish",
    "sv": "Swedish",
    "zh": "Chinese",
}


def load_po_preparer():
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location("shroudforge_po_preparer", PO_PREPARER)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Cannot load the PO formatter: {PO_PREPARER}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def main() -> None:
    if not TRANSLATIONS_PATH.is_file():
        print("No offline translation data has been added yet.")
        return

    preparer = load_po_preparer()
    translations = json.loads(TRANSLATIONS_PATH.read_text(encoding="utf-8"))
    generated = 0

    for article_id, languages in translations.items():
        article_dir = OUTPUT / article_id
        base_path = article_dir / "de.po"
        if not base_path.is_file():
            raise FileNotFoundError(f"Base PO does not exist: {base_path}")
        base_text = base_path.read_text(encoding="utf-8")
        blocks = preparer.re.split(r"\n\s*\n", base_text.rstrip())
        entries = [preparer.parse_po_block(block) for block in blocks if "msgctxt " in block]

        for language, values in languages.items():
            if language not in LANGUAGE_LABELS:
                raise ValueError(f"Unsupported target locale in translation data: {language}")
            if len(values) != len(entries):
                raise ValueError(
                    f"{article_id}/{language} has {len(values)} translations for {len(entries)} PO entries"
                )
            translations_by_context = {
                entry["msgctxt"]: entry["msgid"] if value is None else value
                for entry, value in zip(entries, values, strict=True)
            }
            target = preparer.render_po(
                base_text,
                translations_by_context,
                language,
                LANGUAGE_LABELS[language],
            )
            target_path = article_dir / f"{language}.po"
            target_path.write_text(target, encoding="utf-8", newline="\n")
            generated += 1

    print(f"Generated {generated} PO files from local translated text.")


if __name__ == "__main__":
    main()
