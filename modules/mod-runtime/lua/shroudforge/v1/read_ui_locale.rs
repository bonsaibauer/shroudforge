use std::path::Path;

/// Language the player chose for ShroudForge (`general.locale`, such as "en",
/// "de" or "fr"), so mods can speak it; "en" when the setting is unreadable.
pub(crate) fn read_ui_locale(game_dir: &Path) -> String {
    shroudforge_loader_config::config::read_loader(game_dir)
        .ok()
        .and_then(|config| config["general"]["locale"].as_str().map(str::to_owned))
        .filter(|locale| !locale.is_empty())
        .unwrap_or_else(|| "en".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    /// A game folder whose loader config holds `config`.
    fn game_dir(config: &[u8]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("shroudforge-ui-locale-{}", uuid::Uuid::new_v4()));
        let config_path = shroudforge_loader_config::paths::loader_config(&root);
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(&config_path, config).unwrap();
        root
    }

    #[test]
    fn reads_the_chosen_locale_or_the_default() {
        let chosen = game_dir(br#"{"schemaVersion":1,"general":{"locale":"fr"}}"#);
        assert_eq!(read_ui_locale(&chosen), "fr");
        fs::remove_dir_all(chosen).unwrap();

        // Without a saved choice, the loader default applies.
        let unset = game_dir(br#"{"schemaVersion":1}"#);
        let default = shroudforge_loader_config::config::read_loader_defaults().unwrap();
        assert_eq!(read_ui_locale(&unset), default["general"]["locale"].as_str().unwrap());
        fs::remove_dir_all(unset).unwrap();
    }

    #[test]
    fn falls_back_to_english_when_the_config_is_unreadable() {
        let broken = game_dir(b"not json");
        assert_eq!(read_ui_locale(&broken), "en");
        fs::remove_dir_all(broken).unwrap();
    }
}
