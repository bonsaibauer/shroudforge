use super::*;

#[test]
fn recurring_news_becomes_unread_and_can_be_acknowledged_again() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path();
    crate::config::write_document(root,crate::documents::NEWS_STATE,&serde_json::json!({
            "read":["shroudforge-support-2","shroudforge-welcome-1.6.5"],
            "readAt":{"shroudforge-support-2":now()-91*86400,"shroudforge-welcome-1.6.5":now()-91*86400}
        })).unwrap();
    assert_eq!(read_ids(root).unwrap(), vec!["shroudforge-welcome-1.6.5"]);
    mark_read(root, &["shroudforge-support-2".into()]).unwrap();
    assert_eq!(
        read_ids(root).unwrap(),
        vec!["shroudforge-support-2", "shroudforge-welcome-1.6.5"]
    );
}

#[test]
fn invalid_state_is_reported_and_preserved() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path();
    crate::config::write_json(
        &crate::paths::state_file(root),
        &serde_json::json!({"schemaVersion":1,"news":{"read":[],"readAt":false}}),
    )
    .unwrap();
    assert!(read_ids(root).is_err());
}

#[test]
fn target_read_state_updates_only_the_selected_target() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path();
    let local_id = "local-notice".to_owned();
    let server_id = "server-notice".to_owned();

    mark_news_read_for_target(root, &[local_id.clone()], false).unwrap();
    mark_news_read_for_target(root, &[server_id.clone()], true).unwrap();

    let state = crate::config::read_document(root, crate::documents::NEWS_STATE).unwrap();
    assert_eq!(state["read"], serde_json::json!([local_id.clone()]));
    assert_eq!(state["serverRead"], serde_json::json!([server_id.clone()]));

    mark_news_unread_for_target(root, &[local_id.clone()], false).unwrap();
    mark_news_unread_for_target(root, &[server_id.clone()], true).unwrap();

    let state = crate::config::read_document(root, crate::documents::NEWS_STATE).unwrap();
    assert_eq!(state["read"], serde_json::json!([]));
    assert_eq!(state["serverRead"], serde_json::json!([]));
    assert!(state["readAt"][local_id.as_str()].is_null());
    assert!(state["serverReadAt"][server_id.as_str()].is_null());
}
