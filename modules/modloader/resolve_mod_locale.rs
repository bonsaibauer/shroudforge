use crate::{ModAction, ModGroupLocale, ModLocaleCatalog, ModManifest};
use serde_json::Value;
use std::collections::BTreeMap;

/// Applies the best matching mod catalog to the source manifest.
/// Regional catalogs override the language catalog, and missing values keep
/// the source text from mod.json or extended.mod.json.
pub fn localized_manifest(manifest: &ModManifest, locale: &str) -> ModManifest {
    let mut localized = manifest.clone();
    for code in locale_candidates(locale).into_iter().rev() {
        if let Some(catalog) = manifest.locales.get(&code) {
            apply_catalog(&mut localized, catalog);
        }
    }
    localized
}

/// Looks up a Lua-facing string in the same catalog used to localize the
/// Modloader manifest.
pub fn localized_string(
    catalogs: &BTreeMap<String, ModLocaleCatalog>,
    key: &str,
    locale: &str,
    fallback: Option<&str>,
) -> String {
    for code in locale_candidates(locale) {
        if let Some(value) = catalogs
            .get(&code)
            .and_then(|catalog| catalog.strings.get(key))
        {
            return value.clone();
        }
    }
    fallback.unwrap_or(key).to_owned()
}

/// Ensures translations only refer to controls and actions declared by the
/// source manifest. Group translations follow the source group's array order.
pub fn validate_mod_locale_catalogs(manifest: &ModManifest) -> Result<(), String> {
    for (locale, catalog) in &manifest.locales {
        for (key, translated) in &catalog.settings {
            let setting = manifest
                .settings
                .iter()
                .find(|setting| setting.key == *key)
                .ok_or_else(|| format!("locales/{locale}.json: unknown setting '{key}'"))?;
            for option in translated.options.keys() {
                if !setting
                    .options
                    .iter()
                    .any(|candidate| option_key(&candidate.value) == *option)
                {
                    return Err(format!(
                        "locales/{locale}.json: unknown option '{option}' for setting '{key}'"
                    ));
                }
            }
        }
        for (index, translated) in catalog.groups.iter().enumerate() {
            let group = manifest.groups.get(index).ok_or_else(|| {
                format!(
                    "locales/{locale}.json: group {} has no matching group",
                    index + 1
                )
            })?;
            for action in translated.actions.keys() {
                if !group
                    .actions
                    .iter()
                    .any(|candidate| candidate.id == *action)
                {
                    return Err(format!(
                        "locales/{locale}.json: unknown action '{action}' in group {}",
                        index + 1
                    ));
                }
            }
        }
        if catalog
            .changelog
            .as_ref()
            .is_some_and(|items| items.len() > manifest.changelog.len())
        {
            return Err(format!(
                "locales/{locale}.json: changelog has more entries than extended.mod.json"
            ));
        }
    }
    Ok(())
}

fn locale_candidates(locale: &str) -> Vec<String> {
    if locale == "en" {
        return vec!["en-GB".to_owned(), "en".to_owned()];
    }
    match locale.split_once('-') {
        Some((language, _)) if language != locale => {
            vec![locale.to_owned(), language.to_owned()]
        }
        _ => vec![locale.to_owned()],
    }
}

fn apply_catalog(manifest: &mut ModManifest, catalog: &ModLocaleCatalog) {
    if let Some(name) = &catalog.mod_info.name {
        manifest.name.clone_from(name);
    }
    if let Some(description) = &catalog.mod_info.description {
        manifest.description = Some(description.clone());
    }
    for (key, translated) in &catalog.settings {
        if let Some(setting) = manifest
            .settings
            .iter_mut()
            .find(|setting| setting.key == *key)
        {
            if let Some(label) = &translated.label {
                setting.label.clone_from(label);
            }
            if let Some(description) = &translated.description {
                setting.description = Some(description.clone());
            }
            for option in &mut setting.options {
                if let Some(label) = translated.options.get(&option_key(&option.value)) {
                    option.label.clone_from(label);
                }
            }
        }
    }
    for (index, translated) in catalog.groups.iter().enumerate() {
        if let Some(group) = manifest.groups.get_mut(index) {
            apply_group(group, translated);
        }
    }
    if let Some(changelog) = &catalog.changelog {
        for (target, translated) in manifest.changelog.iter_mut().zip(changelog) {
            target.clone_from(translated);
        }
    }
}

fn apply_group(group: &mut crate::SettingGroup, translated: &ModGroupLocale) {
    if let Some(label) = &translated.label {
        group.label.clone_from(label);
    }
    if let Some(description) = &translated.description {
        group.description = Some(description.clone());
    }
    for action in &mut group.actions {
        apply_action(action, translated);
    }
}

fn apply_action(action: &mut ModAction, translated: &ModGroupLocale) {
    if let Some(localized) = translated.actions.get(&action.id) {
        if let Some(label) = &localized.label {
            action.label.clone_from(label);
        }
        if let Some(confirm) = &localized.confirm {
            action.confirm = Some(confirm.clone());
        }
    }
}

fn option_key(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        value => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::localized_string;
    use crate::ModLocaleCatalog;
    use std::collections::BTreeMap;

    #[test]
    fn english_fallback_prefers_the_modloader_default_locale() {
        let catalogs = BTreeMap::from([
            (
                "en-GB".to_owned(),
                ModLocaleCatalog {
                    strings: BTreeMap::from([("greeting".to_owned(), "Hello from UK".to_owned())]),
                    ..Default::default()
                },
            ),
            (
                "en-US".to_owned(),
                ModLocaleCatalog {
                    strings: BTreeMap::from([("greeting".to_owned(), "Hello from US".to_owned())]),
                    ..Default::default()
                },
            ),
            (
                "en".to_owned(),
                ModLocaleCatalog {
                    strings: BTreeMap::from([(
                        "greeting".to_owned(),
                        "Hello from base English".to_owned(),
                    )]),
                    ..Default::default()
                },
            ),
        ]);

        assert_eq!(
            localized_string(&catalogs, "greeting", "en", None),
            "Hello from UK"
        );
    }
}
