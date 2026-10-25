use serde::{Deserialize, Serialize};
use shroudforge_logging::LogLevel;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub id: String,
    pub mod_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ecosystem: Option<String>,
    pub title: String,
    pub message: String,
    pub level: LogLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beta: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub english_only: Option<bool>,
    pub action_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_label_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<NoticeLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner: Option<NoticeBanner>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<NoticeAction>,
    pub updated_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changelog: Vec<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeAction {
    pub id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeLink {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wiki_article_id: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeBanner {
    pub image_url: String,
    pub link_url: String,
    pub alt: String,
}

/// Mod metadata needed to associate notifications with an installed mod.
pub struct ModNotification<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub version: &'a str,
    pub ecosystem: &'a str,
    pub changelog: &'a [String],
}
