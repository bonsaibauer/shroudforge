//! Portable format validation shared with ShroudEdit through a pinned source/schema snapshot.
use serde::Deserialize;
use std::sync::LazyLock;

pub const MAX_BYTES: usize = 256 * 1024 * 1024;
pub const SCHEMA: &str = include_str!("schemas/8/manifest.schema.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Blueprint {
    #[serde(rename = "$schema")]
    _schema: Option<String>,
    format_version: f64,
    rotation_axis: String,
    extent: [f64; 3],
    voxels: Option<Voxels>,
    props: Vec<Prop>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Voxels {
    dimensions: [f64; 3],
    offset: [f64; 3],
    cell_size: [f64; 3],
    cells: Vec<f64>,
    coverage: Vec<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Prop {
    item_id: f64,
    position: [f64; 3],
    rotation: [f64; 4],
    scale: [f64; 3],
    template_uuid_high_hex: String,
    template_uuid_low_hex: String,
}

pub struct BlueprintMetadata {
    pub extent: [f64; 3],
    pub voxel_dimensions: Option<[u32; 3]>,
}

pub fn validate(bytes: &[u8]) -> Result<BlueprintMetadata, String> {
    if bytes.len() > MAX_BYTES {
        return Err("blueprint exceeds the 256 MiB file limit".into());
    }
    static VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
        jsonschema::validator_for(
            &serde_json::from_str::<serde_json::Value>(SCHEMA)
                .expect("bundled blueprint schema is JSON"),
        )
        .expect("bundled blueprint schema is valid")
    });
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    VALIDATOR
        .validate(&value)
        .map_err(|error| format!("invalid SFBP 8 at {}: {error}", error.instance_path))?;
    drop(value);
    // Typed deserialization additionally rejects duplicate keys at every object level.
    let blueprint: Blueprint = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if blueprint.format_version != 8.0
        || !matches!(blueprint.rotation_axis.as_str(), "x" | "y" | "z")
    {
        return Err("unsupported blueprint version or axis".into());
    }
    let voxel_dimensions = if let Some(voxels) = blueprint.voxels {
        let dimensions = voxels.dimensions.map(|value| value as u32);
        let count = dimensions
            .iter()
            .try_fold(1usize, |count, size| count.checked_mul(*size as usize))
            .filter(|count| *count <= 65_536)
            .ok_or("blueprint exceeds the voxel cell limit")?;
        if voxels.cells.len() != count || voxels.coverage.len() != count {
            return Err("blueprint cell and coverage counts must match dimensions".into());
        }
        if voxels
            .offset
            .iter()
            .chain(&voxels.cell_size)
            .any(|number| !number.is_finite())
        {
            return Err("blueprint voxel transform must be finite".into());
        }
        if voxels
            .cells
            .iter()
            .zip(&voxels.coverage)
            .any(|(cell, coverage)| *coverage == 2.0 && *cell == 0.0)
        {
            return Err("occupied blueprint cells must have a nonzero value".into());
        }
        Some(dimensions)
    } else {
        None
    };
    for prop in blueprint.props {
        let norm = prop.rotation.iter().map(|value| value * value).sum::<f64>();
        if !norm.is_finite()
            || norm < 1e-12
            || !prop.item_id.is_finite()
            || prop
                .position
                .iter()
                .chain(&prop.scale)
                .any(|value| !value.is_finite())
        {
            return Err("blueprint prop transform is invalid".into());
        }
        if prop.template_uuid_high_hex.bytes().all(|byte| byte == b'0')
            && prop.template_uuid_low_hex.bytes().all(|byte| byte == b'0')
        {
            return Err("blueprint template UUID must be nonzero".into());
        }
    }
    Ok(BlueprintMetadata {
        extent: blueprint.extent,
        voxel_dimensions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blueprint_contract_preserves_dimensions_and_rejects_semantic_mismatches() {
        let example = include_bytes!("schemas/8/manifest.example.json");
        assert_eq!(validate(example).unwrap().voxel_dimensions, Some([2, 1, 3]));
        let source: serde_json::Value = serde_json::from_slice(example).unwrap();
        for (pointer, replacement) in [
            ("/voxels/dimensions/0", serde_json::json!(3)),
            ("/voxels/cells/0", serde_json::json!(0)),
            ("/props/0/rotation", serde_json::json!([0, 0, 0, 0])),
            ("/props/0/templateUuidHighHex", serde_json::json!(1234)),
            ("/formatVersion", serde_json::json!(7)),
            ("/voxels", serde_json::Value::Null),
        ] {
            let mut invalid = source.clone();
            *invalid.pointer_mut(pointer).unwrap() = replacement;
            assert!(
                validate(&serde_json::to_vec(&invalid).unwrap()).is_err(),
                "{pointer}"
            );
        }
        assert!(validate(br#"{"formatVersion":8,"formatVersion":8,"rotationAxis":"y","extent":[1,1,1],"props":[]}"#).is_err());
        assert!(
            validate(br#"{"formatVersion":8,"rotationAxis":"y","extent":[1,1,1],"props":[]}"#)
                .is_ok()
        );
    }
}
