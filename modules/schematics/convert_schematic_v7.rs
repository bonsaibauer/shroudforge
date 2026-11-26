use serde_json::{Value, json};

use crate::validate_schematic_v8;

const V7_MARKER: &str = "SHROUDFORGE_WORLD_BLUEPRINT_V7";
const V8_SCHEMA: &str = "https://shroudforge.net/schemas/sfbp/8/manifest.schema.json";
const MAX_PROPS: u64 = 1_000_000;

#[derive(Clone, Debug)]
pub struct Inspection {
    pub format_version: Option<u32>,
    pub valid: bool,
    pub convertible: bool,
    pub validation_error: Option<String>,
}

pub fn inspect(bytes: &[u8]) -> Inspection {
    let format_version = detect_version(bytes);
    if bytes.len() > validate_schematic_v8::MAX_BYTES {
        return Inspection {
            format_version,
            valid: false,
            convertible: false,
            validation_error: Some("Schematic exceeds the 256 MiB size limit".into()),
        };
    }

    match format_version {
        Some(8) => match validate_schematic_v8::validate(bytes) {
            Ok(_) => Inspection {
                format_version,
                valid: true,
                convertible: false,
                validation_error: None,
            },
            Err(error) => Inspection {
                format_version,
                valid: false,
                convertible: false,
                validation_error: Some(error),
            },
        },
        Some(7) => match convert_v7_to_v8(bytes) {
            Ok(_) => Inspection {
                format_version,
                valid: false,
                convertible: true,
                validation_error: None,
            },
            Err(error) => Inspection {
                format_version,
                valid: false,
                convertible: false,
                validation_error: Some(error),
            },
        },
        Some(version) => Inspection {
            format_version,
            valid: false,
            convertible: false,
            validation_error: Some(format!(
                "Unsupported SFBP v{version}. This build reads v8 and can convert v7."
            )),
        },
        None => Inspection {
            format_version,
            valid: false,
            convertible: false,
            validation_error: Some("The file is not a recognized SFBP v7 or v8 file".into()),
        },
    }
}

pub fn detect_version(bytes: &[u8]) -> Option<u32> {
    if let Ok(text) = std::str::from_utf8(bytes) {
        if let Some(first_line) = text.split(['\r', '\n']).next()
            && let Some(version) = first_line.strip_prefix("SHROUDFORGE_WORLD_BLUEPRINT_V")
            && !version.is_empty()
            && version.bytes().all(|byte| byte.is_ascii_digit())
        {
            return version.parse().ok();
        }
    }

    let value: Value = serde_json::from_slice(bytes).ok()?;
    let version = value.get("formatVersion")?.as_f64()?;
    (version.is_finite() && version.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&version))
        .then_some(version as u32)
}

pub fn convert_v7_to_v8(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.is_empty() {
        return Err("SFBP file is empty".into());
    }
    if bytes.len() > validate_schematic_v8::MAX_BYTES {
        return Err("Schematic exceeds the 256 MiB size limit".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "SFBP v7 must be UTF-8 text")?;
    let lines: Vec<_> = text
        .split_terminator('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    if lines.len() < 10 || lines[0] != V7_MARKER {
        return Err("SFBP v7 header or required fields are missing".into());
    }

    let rotation_axis = lines[1];
    if !matches!(rotation_axis, "x" | "y" | "z") {
        return Err("SFBP v7 rotation axis is invalid".into());
    }
    let extent = parse_triplet(lines[2], "extent")?;
    let kind = lines[3];
    if !matches!(kind, "voxel" | "props") {
        return Err("SFBP v7 blueprint channel is invalid".into());
    }
    let has_voxels = kind == "voxel";
    if extent.iter().any(|value| *value < 0.0)
        || (!has_voxels && extent.iter().any(|value| *value <= 0.0))
    {
        return Err("SFBP v7 extent is invalid".into());
    }

    let mut blueprint = json!({
        "$schema": V8_SCHEMA,
        "formatVersion": 8,
        "rotationAxis": rotation_axis,
        "extent": extent,
        "props": [],
    });

    if has_voxels {
        let dimensions = parse_triplet(lines[4], "voxel dimensions")?;
        let dimensions = dimensions
            .iter()
            .enumerate()
            .map(|(index, value)| {
                integer_value(*value, 1, 65_536)
                    .map_err(|_| format!("SFBP v7 voxel dimension {} is invalid", index + 1))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let count = dimensions
            .iter()
            .try_fold(1_u64, |count, dimension| count.checked_mul(*dimension))
            .filter(|count| *count <= 65_536)
            .ok_or("SFBP v7 exceeds the 65,536 voxel cell limit")?;
        let offset = parse_triplet(lines[5], "voxel offset")?;
        let cell_size = parse_triplet(lines[6], "voxel cell size")?;
        if cell_size.iter().any(|value| *value <= 0.0) {
            return Err("SFBP v7 voxel cell size must be positive".into());
        }
        let cells = parse_integer_list(lines[7], "voxel cells", 0, 65_535)?;
        if cells.len() as u64 != count {
            return Err("SFBP v7 voxel cell count does not match its dimensions".into());
        }
        let mut coverage = parse_integer_list(lines[8], "voxel coverage", 0, 2)?;
        if coverage.len() as u64 != count {
            return Err("SFBP v7 voxel coverage count does not match its dimensions".into());
        }
        let mut cells = cells;
        for (index, state) in coverage.iter_mut().enumerate() {
            if *state == 2 && cells[index] == 0 {
                return Err("Occupied SFBP v7 cells must have a nonzero value".into());
            }
            if *state != 2 {
                cells[index] = 0;
            }
        }
        blueprint["voxels"] = json!({
            "dimensions": dimensions,
            "offset": offset,
            "cellSize": cell_size,
            "cells": cells,
            "coverage": coverage,
        });
    } else if [lines[4], lines[5], lines[6], lines[7], lines[8]]
        .iter()
        .any(|value| *value != "-")
    {
        return Err("SFBP v7 props-only files must not contain voxel data".into());
    }

    let prop_count = parse_integer(lines[9], "prop count", 0, MAX_PROPS)?;
    if lines.len() as u64 != 10 + prop_count {
        return Err("SFBP v7 prop count does not match the file".into());
    }
    let mut props = Vec::with_capacity(prop_count as usize);
    for line in &lines[10..] {
        props.push(parse_prop(line)?);
    }
    blueprint["props"] = Value::Array(props);

    let encoded = serde_json::to_vec(&blueprint)
        .map_err(|error| format!("Could not encode SFBP v8: {error}"))?;
    validate_schematic_v8::validate(&encoded)?;
    Ok(encoded)
}

fn parse_triplet(text: &str, label: &str) -> Result<[f64; 3], String> {
    let fields: Vec<_> = text.split(',').collect();
    if fields.len() != 3 {
        return Err(format!("SFBP v7 {label} must contain three values"));
    }
    let values = fields
        .iter()
        .map(|field| parse_number(field, label))
        .collect::<Result<Vec<_>, _>>()?;
    Ok([values[0], values[1], values[2]])
}

fn parse_number(text: &str, label: &str) -> Result<f64, String> {
    let value = text
        .parse::<f64>()
        .map_err(|_| format!("SFBP v7 {label} contains an invalid number"))?;
    if !value.is_finite() {
        return Err(format!("SFBP v7 {label} must contain finite numbers"));
    }
    Ok(value)
}

fn integer_value(value: f64, minimum: u64, maximum: u64) -> Result<u64, String> {
    if !value.is_finite()
        || value.fract() != 0.0
        || value < minimum as f64
        || value > maximum as f64
    {
        return Err("value is not a supported integer".into());
    }
    Ok(value as u64)
}

fn parse_integer(text: &str, label: &str, minimum: u64, maximum: u64) -> Result<u64, String> {
    let value = parse_number(text, label)?;
    integer_value(value, minimum, maximum)
        .map_err(|_| format!("SFBP v7 {label} is outside the supported integer range"))
}

fn parse_integer_list(
    text: &str,
    label: &str,
    minimum: u64,
    maximum: u64,
) -> Result<Vec<u64>, String> {
    if text.is_empty() {
        return Err(format!("SFBP v7 {label} are missing"));
    }
    text.split(',')
        .map(|field| parse_integer(field, label, minimum, maximum))
        .collect()
}

fn parse_prop(line: &str) -> Result<Value, String> {
    let fields: Vec<_> = line.split(',').collect();
    if fields.len() != 13 {
        return Err("SFBP v7 prop must contain thirteen fields".into());
    }
    let item_id = parse_integer(fields[0], "prop item ID", 1, u32::MAX as u64)?;
    let numbers = fields[1..11]
        .iter()
        .map(|field| parse_number(field, "prop transform"))
        .collect::<Result<Vec<_>, _>>()?;
    let rotation_norm = numbers[3..7].iter().map(|value| value * value).sum::<f64>();
    if !rotation_norm.is_finite() || rotation_norm < 1e-12 {
        return Err("SFBP v7 prop rotation is invalid".into());
    }
    let high = validate_uuid_half(fields[11])?;
    let low = validate_uuid_half(fields[12])?;
    if high.bytes().all(|byte| byte == b'0') && low.bytes().all(|byte| byte == b'0') {
        return Err("SFBP v7 prop template UUID must be nonzero".into());
    }
    Ok(json!({
        "itemId": item_id,
        "position": [numbers[0], numbers[1], numbers[2]],
        "rotation": [numbers[3], numbers[4], numbers[5], numbers[6]],
        "scale": [numbers[7], numbers[8], numbers[9]],
        "templateUuidHighHex": high,
        "templateUuidLowHex": low,
    }))
}

fn validate_uuid_half(value: &str) -> Result<String, String> {
    if value.is_empty() || value.len() > 16 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("SFBP v7 prop template UUID is invalid".into());
    }
    Ok(value.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    const V7_PROPS: &[u8] = b"SHROUDFORGE_WORLD_BLUEPRINT_V7\ny\n1,1,1\nprops\n-\n-\n-\n-\n-\n1\n42,0.5,0.5,0.5,0,0,0,1,1,1,1,FEDCBA9876543210,0123456789ABCDEF";

    #[test]
    fn converts_v7_props_without_changing_uuid_or_transforms() {
        let converted = convert_v7_to_v8(V7_PROPS).unwrap();
        let value: Value = serde_json::from_slice(&converted).unwrap();
        assert_eq!(value["formatVersion"], 8);
        assert_eq!(value["props"][0]["itemId"], 42);
        assert_eq!(value["props"][0]["position"], json!([0.5, 0.5, 0.5]));
        assert_eq!(value["props"][0]["templateUuidHighHex"], "fedcba9876543210");
        assert_eq!(value["props"][0]["templateUuidLowHex"], "0123456789abcdef");
        assert!(validate_schematic_v8::validate(&converted).is_ok());
    }

    #[test]
    fn converts_v7_voxels_and_coverage() {
        let legacy =
            b"SHROUDFORGE_WORLD_BLUEPRINT_V7\nx\n1,1,1\nvoxel\n1,1,1\n0,0,0\n1,1,1\n42\n2\n0";
        let converted = convert_v7_to_v8(legacy).unwrap();
        let value: Value = serde_json::from_slice(&converted).unwrap();
        assert_eq!(value["voxels"]["dimensions"], json!([1, 1, 1]));
        assert_eq!(value["voxels"]["cells"], json!([42]));
        assert_eq!(value["voxels"]["coverage"], json!([2]));
        assert!(validate_schematic_v8::validate(&converted).is_ok());
    }

    #[test]
    fn inspects_current_older_and_unrecognized_schematic_files() {
        let older = inspect(V7_PROPS);
        assert_eq!(older.format_version, Some(7));
        assert!(older.convertible);
        assert!(!older.valid);

        let current = br#"{"formatVersion":8,"rotationAxis":"y","extent":[1,1,1],"props":[]}"#;
        let current = inspect(current);
        assert_eq!(current.format_version, Some(8));
        assert!(current.valid);
        assert!(!current.convertible);

        let unknown = inspect(b"not an SFBP file");
        assert_eq!(unknown.format_version, None);
        assert!(!unknown.valid);
    }

    #[test]
    fn rejects_malformed_v7_instead_of_silently_converting_it() {
        let invalid = std::str::from_utf8(V7_PROPS)
            .unwrap()
            .replace("\n1\n42,", "\n2\n42,");
        assert!(convert_v7_to_v8(invalid.as_bytes()).is_err());
        let status = inspect(invalid.as_bytes());
        assert!(!status.convertible);
    }
}
