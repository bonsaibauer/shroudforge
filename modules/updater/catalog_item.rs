use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItem {
    pub id: String,
    pub slug: String,
    pub project_type: String,
    pub project_types: Vec<String>,
    pub name: String,
    pub summary: String,
    pub icon_url: Option<String>,
    pub author: Option<String>,
    pub downloads: Option<u64>,
    pub follows: Option<u64>,
    pub version: Option<String>,
    pub environment: Option<String>,
    pub date_modified: Option<String>,
    pub latest_version_id: Option<String>,
    pub loaders: Vec<String>,
    pub categories: Vec<String>,
    pub game_versions: Vec<String>,
    pub gallery: Vec<String>,
    pub installed_mod_id: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSearchResult {
    pub query: String,
    pub project_type: String,
    pub project_types: Vec<String>,
    pub loader: Vec<String>,
    pub category: Vec<String>,
    pub game_version: Vec<String>,
    pub sort: String,
    pub page: usize,
    pub limit: usize,
    pub total: usize,
    pub items: Vec<CatalogItem>,
    pub categories: Vec<String>,
    pub game_versions: Vec<String>,
}
