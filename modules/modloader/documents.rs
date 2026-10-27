use shroudforge_loader_config::{Document, Storage};
pub const MOD: Document = Document {
    name: "mod",
    schema: include_str!("manifest/manifest.schema.json"),
    storage: Storage::Package,
};
pub const EXTENDED_MOD: Document = Document {
    name: "extended-mod",
    schema: include_str!("manifest/extended.mod.schema.json"),
    storage: Storage::Config("extended-mod/extended-mod.json"),
};
pub const MOD_LOCALE: Document = Document {
    name: "mod-locale",
    schema: include_str!("manifest/mod-locale.schema.json"),
    storage: Storage::Package,
};
pub const RULES: Document = Document {
    name: "rules",
    schema: include_str!("conflicts/rules.schema.json"),
    storage: Storage::Config("compatibility/rules.json"),
};
