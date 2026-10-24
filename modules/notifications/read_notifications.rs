use crate::{ModNotification, Notice, NoticeAction, NoticeBanner, NoticeLink};
use shroudforge_logging::LogLevel;
use std::path::Path;

pub fn normalize_legacy_event_level(mut value: serde_json::Value) -> serde_json::Value {
    let replacement = match value.get("level").and_then(|level| level.as_str()) {
        Some("warning") => Some("warn"),
        Some("success" | "update") => Some("info"),
        _ => None,
    };
    if let (Some(level), Some(object)) = (replacement, value.as_object_mut()) {
        object.insert("level".into(), serde_json::Value::String(level.into()));
    }
    value
}

pub fn read_notifications(root: &Path, mods: &[ModNotification<'_>]) -> Vec<Notice> {
    let events =
        crate::config::read_document(root, crate::documents::EVENTS_STATE).unwrap_or_default();
    let news = crate::read(root)
        .ok()
        .and_then(|value| value.get("messages").and_then(|v| v.as_array()).cloned())
        .unwrap_or_default();
    let event_notices = events
        .as_object()
        .into_iter()
        .flat_map(|values| values.values().cloned())
        .filter_map(|value| {
            let normalized = normalize_legacy_event_level(value.clone());
            (crate::validate_event(root, &normalized).is_ok()
                || crate::validate_event(root, &value).is_ok())
            .then_some(normalized)
        })
        .collect::<Vec<_>>();
    let mut notices = event_notices
        .into_iter()
        .chain(news)
        .filter_map(|value| {
            let mod_id = value.get("modId").and_then(|value| value.as_str());
            Some(Notice {
                id: value.get("id")?.as_str()?.to_owned(),
                mod_id: mod_id.unwrap_or("shroudforge").to_owned(),
                ecosystem: value
                    .get("ecosystem")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned)
                    .or_else(|| {
                        let mod_id = mod_id?;
                        mods.iter()
                            .find(|item| item.id == mod_id)
                            .map(|item| item.ecosystem.to_owned())
                    }),
                title: value.get("title")?.as_str()?.chars().take(120).collect(),
                message: value
                    .get("message")?
                    .as_str()?
                    .chars()
                    .take(2_000)
                    .collect(),
                level: value
                    .get("level")
                    .and_then(|value| value.as_str())
                    .and_then(LogLevel::parse)
                    .unwrap_or(LogLevel::Info),
                beta: value.get("beta").and_then(|value| value.as_bool()),
                action_url: value
                    .get("actionUrl")
                    .and_then(|value| value.as_str())
                    .filter(|url| url.starts_with("https://"))
                    .map(str::to_owned),
                action_label_key: value
                    .get("actionLabelKey")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned),
                links: value
                    .get("links")
                    .and_then(|value| value.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|link| {
                        let url = link
                            .get("url")
                            .and_then(|value| value.as_str())
                            .filter(|url| url.starts_with("https://"))
                            .map(str::to_owned);
                        let wiki_article_id = link
                            .get("wikiArticleId")
                            .and_then(|value| value.as_str())
                            .filter(|id| {
                                !id.is_empty()
                                    && id.len() <= 80
                                    && id.bytes().all(|byte| {
                                        byte.is_ascii_lowercase()
                                            || byte.is_ascii_digit()
                                            || byte == b'-'
                                    })
                            })
                            .map(str::to_owned);
                        if url.is_some() == wiki_article_id.is_some() {
                            return None;
                        }
                        let label_key = link
                            .get("labelKey")
                            .and_then(|value| value.as_str())
                            .filter(|value| !value.is_empty())
                            .map(|value| value.chars().take(120).collect());
                        let label = link
                            .get("label")
                            .and_then(|value| value.as_str())
                            .filter(|value| !value.is_empty())
                            .map(|value| value.chars().take(120).collect());
                        (label_key.is_some() || label.is_some()).then(|| NoticeLink {
                            label_key,
                            label,
                            url,
                            wiki_article_id,
                        })
                    })
                    .take(4)
                    .collect(),
                banner: value.get("banner").and_then(|banner| {
                    let image_url = banner
                        .get("imageUrl")
                        .and_then(|value| value.as_str())
                        .filter(|url| url.starts_with("https://"))?;
                    let link_url = banner
                        .get("linkUrl")
                        .and_then(|value| value.as_str())
                        .filter(|url| url.starts_with("https://"))?;
                    let alt = banner
                        .get("alt")
                        .and_then(|value| value.as_str())
                        .filter(|value| !value.is_empty())?;
                    Some(NoticeBanner {
                        image_url: image_url.to_owned(),
                        link_url: link_url.to_owned(),
                        alt: alt.chars().take(120).collect(),
                    })
                }),
                actions: value
                    .get("actions")
                    .and_then(|value| value.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|action| {
                        let kind = action.get("kind")?.as_str()?;
                        if !["open-mod", "open-debug-console", "open-installed-mods"]
                            .contains(&kind)
                        {
                            return None;
                        }
                        Some(NoticeAction {
                            id: action
                                .get("id")
                                .and_then(|value| value.as_str())
                                .unwrap_or(kind)
                                .to_owned(),
                            kind: kind.to_owned(),
                            target: action
                                .get("target")
                                .and_then(|value| value.as_str())
                                .map(str::to_owned),
                        })
                    })
                    .take(4)
                    .collect(),
                updated_at: value
                    .get("updatedAt")
                    .and_then(|value| value.as_u64())
                    .unwrap_or_default(),
                kind: value
                    .get("kind")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned),
                values: value.get("values").cloned(),
                changelog: value
                    .get("changelog")
                    .and_then(|value| value.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.as_str().map(str::to_owned))
                            .take(50)
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    notices.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    notices.truncate(100);
    notices
}
