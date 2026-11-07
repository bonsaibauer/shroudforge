use crate::{SettingCondition, SettingDefinition, SettingOption, SettingValueType};
use serde_json::{Value, json};
pub(crate) fn parse_setting_definition(
    key: &str,
    value: &Value,
    metadata: &Value,
) -> Result<SettingDefinition, String> {
    let value_type = match value {
        Value::Bool(_) => SettingValueType::Boolean,
        Value::String(_) => SettingValueType::String,
        Value::Number(number) if number.as_i64().is_some() => SettingValueType::Integer,
        Value::Number(_) => SettingValueType::Number,
        Value::Array(_) => SettingValueType::Array,
        _ => {
            return Err(format!(
                "setting '{key}' value must be boolean, string, number, or array"
            ));
        }
    };
    let options = metadata
        .get("options")
        .and_then(Value::as_object)
        .map(|entries| {
            entries
                .iter()
                .map(|(option, label)| {
                    let option_value = match value_type {
                        SettingValueType::Boolean => Value::Bool(option == "true"),
                        SettingValueType::Integer => Value::Number(
                            option
                                .parse::<i64>()
                                .map_err(|_| format!("invalid option for '{key}'"))?
                                .into(),
                        ),
                        SettingValueType::Number => json!(
                            option
                                .parse::<f64>()
                                .map_err(|_| format!("invalid option for '{key}'"))?
                        ),
                        SettingValueType::Array => {
                            match value.as_array().and_then(|items| items.first()) {
                                Some(Value::Bool(_)) => Value::Bool(option == "true"),
                                Some(Value::Number(number)) if number.as_i64().is_some() => {
                                    Value::Number(
                                        option
                                            .parse::<i64>()
                                            .map_err(|_| format!("invalid option for '{key}'"))?
                                            .into(),
                                    )
                                }
                                Some(Value::Number(_)) => json!(
                                    option
                                        .parse::<f64>()
                                        .map_err(|_| format!("invalid option for '{key}'"))?
                                ),
                                _ => Value::String(option.clone()),
                            }
                        }
                        _ => Value::String(option.clone()),
                    };
                    Ok(SettingOption {
                        value: option_value,
                        label: label
                            .as_str()
                            .ok_or_else(|| format!("option label for '{key}' must be a string"))?
                            .to_owned(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?
        .unwrap_or_default();
    let mut display_key = String::with_capacity(key.len());
    for (index, character) in key.chars().enumerate() {
        if index > 0 && character.is_uppercase() {
            display_key.push(' ');
        }
        display_key.push(character);
    }
    let mut control = metadata
        .get("control")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| format!("setting '{key}' control: {error}"))?;
    if value_type == SettingValueType::Array && options.is_empty() {
        return Err(format!("array setting '{key}' must declare options"));
    }
    if control.is_none() && !options.is_empty() {
        control = Some(if value_type == SettingValueType::Array {
            crate::SettingControl::Multiselect
        } else {
            crate::SettingControl::Select
        });
    }
    let apply_at_explicit =
        metadata.get("applyAt").is_some() || metadata.get("restartRequired").is_some();
    let apply_at = match metadata.get("applyAt") {
        None if metadata.get("restartRequired") == Some(&Value::Bool(false)) => "live",
        None => "restart",
        Some(Value::String(value)) => value.as_str(),
        Some(_) => return Err(format!("setting '{key}' applyAt must be a string")),
    };
    if !matches!(apply_at, "live" | "restart") {
        return Err(format!(
            "setting '{key}' applyAt must be 'live' or 'restart'"
        ));
    }
    let restart_required = match metadata.get("restartRequired") {
        None => apply_at == "restart",
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err(format!("setting '{key}' restartRequired must be a boolean")),
    };
    if restart_required != (apply_at == "restart") {
        return Err(format!(
            "setting '{key}' applyAt and restartRequired must agree"
        ));
    }
    let asset_preparation = match metadata.get("assetPreparation") {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err(format!("setting '{key}' assetPreparation must be a boolean")),
    };
    let enabled_when = metadata
        .get("enabledWhen")
        .cloned()
        .map(serde_json::from_value::<SettingCondition>)
        .transpose()
        .map_err(|error| format!("setting '{key}' enabledWhen: {error}"))?;
    Ok(SettingDefinition {
        key: key.into(),
        value_type,
        control,
        label: metadata
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or(&display_key)
            .into(),
        description: metadata
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_owned),
        default: value.clone(),
        group: None,
        minimum: metadata.get("min").and_then(Value::as_f64),
        maximum: metadata.get("max").and_then(Value::as_f64),
        step: metadata.get("step").and_then(Value::as_f64),
        minimum_length: metadata
            .get("minLength")
            .and_then(Value::as_u64)
            .map(|value| value as usize),
        maximum_length: metadata
            .get("maxLength")
            .and_then(Value::as_u64)
            .map(|value| value as usize),
        options,
        enabled_when,
        asset_preparation,
        restart_required,
        apply_at: apply_at.into(),
        apply_at_explicit,
        reserve: None,
        reserved: None,
        wheel: None,
    })
}
