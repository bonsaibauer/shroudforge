from concurrent.futures import ProcessPoolExecutor, as_completed
import csv
import json
import os
from pathlib import Path
import re

# --- Configuration ---
ROOT_DIR = Path('.')
TARGET_PREFIX = 'exported_assets'
ARCHIVE_KEYWORD = 'archive'

# Set to True to force re-formatting on ALL files
FORCE_REPROCESS = False


def load_type_mappings() -> dict[str, str]:
  """Finds the latest 0_resource_types_export_*.csv file anywhere in the directory tree

  and builds a mapping of TypeName -> ResolvedName.
  """
  csv_files = sorted(
      ROOT_DIR.rglob('0_resource_types_export_*.csv'), reverse=True
  )
  if not csv_files:
    print(
        "[WARNING] No '0_resource_types_export_*.csv' file found. Folder names"
        ' will be used as fallback for $type.'
    )
    return {}

  target_csv = csv_files[0]
  print(f'Loading type mappings from: {target_csv}')

  type_map = {}
  try:
    with open(target_csv, mode='r', encoding='utf-8-sig') as f:
      reader = csv.DictReader(f)
      for row in reader:
        type_name = row.get('TypeName', '').strip()
        resolved_name = row.get('ResolvedName', '').strip()
        if type_name and resolved_name:
          type_map[type_name] = resolved_name
    print(f'Loaded {len(type_map)} type mappings.\n')
  except Exception as e:
    print(f'[ERROR] Failed to read {target_csv.name}: {e}\n')

  return type_map


def sanitize_malformed_components(text: str) -> str:
  """Converts unquoted key-value pairs inside [Key: Value, Key: Value] arrays

  into proper JSON objects: [{"Key": "Value"}, {"Key": "Value"}].
  """

  def parse_key_value_array(match):
    content = match.group(1)
    items = [item.strip() for item in content.split(',') if item.strip()]
    json_objects = []

    for item in items:
      # If item is in "Key: Value" or "Key:Value" format
      if ':' in item:
        key, val = item.split(':', 1)
        key = key.strip().strip('"\'')
        val = val.strip().strip('"\'')
        json_objects.append(f'{{"{key}": "{val}"}}')
      else:
        # Fallback if an item doesn't have a colon
        json_objects.append(f'"{item}"')

    return '[' + ', '.join(json_objects) + ']'

  # Matches array contents like [MappedVariantValue: 123, MappedVariantValue: 456]
  return re.sub(
      r'\[\s*([a-zA-Z0-9_\$]+\s*:[^\]]+)\]', parse_key_value_array, text
  )


def decode_embedded_json(data):
  """Recursively converts stringified JSON fields into real nested JSON objects."""
  if isinstance(data, dict):
    return {k: decode_embedded_json(v) for k, v in data.items()}
  elif isinstance(data, list):
    return [decode_embedded_json(item) for item in data]
  elif isinstance(data, str):
    cleaned = data.strip()
    if (cleaned.startswith('{') and cleaned.endswith('}')) or (
        cleaned.startswith('[') and cleaned.endswith(']')
    ):
      # Step 1: Fix Key: Value entries missing object brackets and quotes inside arrays
      sanitized = sanitize_malformed_components(cleaned)

      # Step 2: Parse as real JSON
      try:
        parsed = json.loads(sanitized)
        return decode_embedded_json(parsed)
      except (json.JSONDecodeError, TypeError):
        pass

  return data


def process_file(
    file_path: Path, type_map: dict[str, str], force_reprocess: bool = False
) -> str:
  """Processes a single JSON file:

  1. Reads file contents.
  2. Parses stringified JSON and fixes key-value component arrays.
  3. Promotes/unwraps inner fields if top-level 'data' was a stringified dict.
  4. Updates top-level metadata ($type, $guid, $part).
  5. Serializes back to pretty-printed, properly quoted JSON.
  """
  try:
    with open(file_path, 'r', encoding='utf-8') as f:
      raw_content = f.read()
      raw_data = json.loads(raw_content)

    # Step 1: Unwrap & repair embedded JSON strings
    clean_data = decode_embedded_json(raw_data)

    # Step 2: If 'data' was decoded into a dictionary, promote inner keys to top level
    if isinstance(clean_data, dict) and 'data' in clean_data:
      inner_data = clean_data['data']
      if isinstance(inner_data, dict):
        clean_data = inner_data

    # Step 3: Require the current resource object shape.
    if not isinstance(clean_data, dict):
      clean_data = {'data': clean_data}

    # Step 4: Extract $guid and $part from filename
    stem_parts = file_path.stem.split('_')
    guid = stem_parts[0]
    try:
      part = int(stem_parts[-1])
    except ValueError:
      part = stem_parts[-1]

    # Step 5: Derive $type from parent subfolder name via CSV lookup
    subfolder_name = file_path.parent.name
    resolved_type = type_map.get(subfolder_name, subfolder_name)

    # Step 6: Assign top-level metadata tags
    clean_data['$type'] = resolved_type
    clean_data['$guid'] = guid
    clean_data['$part'] = part

    # Step 7: Serialize to pretty-printed standard JSON string
    formatted_json_str = json.dumps(clean_data, indent=2, ensure_ascii=False)

    # Step 8: Skip check (bypassed if FORCE_REPROCESS is True)
    if not force_reprocess and raw_content == formatted_json_str:
      return 'skipped'

    # Step 9: Overwrite file with corrected output
    with open(file_path, 'w', encoding='utf-8') as f:
      f.write(formatted_json_str)

    return 'processed'

  except (json.JSONDecodeError, OSError) as e:
    print(f'\n[ERROR] Failed to process {file_path}: {e}')
    return 'error'


def main():
  type_map = load_type_mappings()

  target_dirs = [
      d
      for d in ROOT_DIR.rglob('*')
      if d.is_dir()
      and d.name.startswith(TARGET_PREFIX)
      and ARCHIVE_KEYWORD not in str(d).lower()
  ]

  if not target_dirs:
    print(
        'No non-archived directories starting with'
        f" '{TARGET_PREFIX}' found under {ROOT_DIR.resolve()}."
    )
    return

  print('Target asset export directories found:')
  for d in target_dirs:
    print(f' - {d}')
  print()

  json_files = []
  for directory in target_dirs:
    for file_path in directory.rglob('*.json'):
      if any(
          ARCHIVE_KEYWORD in parent.name.lower() for parent in file_path.parents
      ):
        continue
      json_files.append(file_path)

  total_files = len(json_files)
  if total_files == 0:
    print('No non-archived JSON files found inside subfolders to process.')
    return

  print(
      f'Found {total_files} JSON file(s) across {len(target_dirs)} export'
      ' folder(s).'
  )
  if FORCE_REPROCESS:
    print(
        '>>> FORCE_REPROCESS is ENABLED: All files will be rewritten and'
        ' formatted.'
    )

  print(
      f'Starting parallel processing using {os.cpu_count() or 4} CPU'
      ' workers...\n'
  )

  processed_count = 0
  skipped_count = 0
  error_count = 0

  with ProcessPoolExecutor() as executor:
    futures = {
        executor.submit(
            process_file, path, type_map, FORCE_REPROCESS
        ): path
        for path in json_files
    }

    for i, future in enumerate(as_completed(futures), start=1):
      status = future.result()

      if status == 'processed':
        processed_count += 1
      elif status == 'skipped':
        skipped_count += 1
      elif status == 'error':
        error_count += 1

      if i % 1000 == 0 or i == total_files:
        percent = (i / total_files) * 100
        print(
            f'Progress: {i}/{total_files} ({percent:.1f}%) | Updated:'
            f' {processed_count} | Skipped: {skipped_count} | Errors:'
            f' {error_count}'
        )

  print('\n--- Processing Complete ---')
  print(f'Total Files Scanned : {total_files}')
  print(f'Files Processed     : {processed_count}')
  print(f'Files Skipped       : {skipped_count}')
  print(f'Errors Encountered  : {error_count}')


if __name__ == '__main__':
  main()
