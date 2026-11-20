//! Best-effort mirror only. Persistence and native presentation are independent.
fn bounded_text(value: &str, bytes: usize) -> &str {
    let mut end = value.len().min(bytes);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn publish_with(
    id: &str,
    title: &str,
    message: &str,
    level: &str,
    send: impl FnOnce(&str, &str, &str, u32) -> i32,
) -> i32 {
    let level = match level {
        "info" => 0,
        "success" => 1,
        "warning" => 2,
        "error" => 3,
        "update" => 4,
        _ => return 1,
    };
    if id.is_empty()
        || id.len() > 256
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'))
        || title.is_empty()
        || message.is_empty()
        || title.contains('\0')
        || message.contains('\0')
    {
        return 1;
    }
    send(
        id,
        bounded_text(title, 120),
        bounded_text(message, 512),
        level,
    )
}

/// Returns queue status only; missing graphics/provider never invalidates feed data.
pub fn publish_notification(id: &str, title: &str, message: &str, level: &str) -> i32 {
    publish_with(
        id,
        title,
        message,
        level,
        crate::native_publish_notification,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_limits_and_level_mapping_are_bounded() {
        let long = "é".repeat(400);
        let result = publish_with(
            "mod.event",
            &long,
            &long,
            "warning",
            |id, title, message, level| {
                assert_eq!(id, "mod.event");
                assert_eq!(title.len(), 120);
                assert_eq!(message.len(), 512);
                assert!(title.ends_with('é') && message.ends_with('é'));
                assert_eq!(level, 2);
                6
            },
        );
        assert_eq!(result, 6);
        assert_eq!(bounded_text("aé", 2), "a");
    }
    #[test]
    fn invalid_inputs_never_reach_native() {
        for (id, title, message, level) in [
            ("", "t", "m", "info"),
            ("bad/id", "t", "m", "info"),
            ("a", "", "m", "info"),
            ("a", "t", "x\0y", "info"),
            ("a", "t", "m", "unknown"),
        ] {
            assert_eq!(
                publish_with(id, title, message, level, |_, _, _, _| panic!(
                    "native called"
                )),
                1
            );
        }
        assert_eq!(
            publish_with(&"a".repeat(257), "t", "m", "info", |_, _, _, _| panic!(
                "native called"
            )),
            1
        );
    }
    #[test]
    fn unavailable_is_a_mirror_result() {
        assert_eq!(publish_with("a", "t", "m", "info", |_, _, _, _| 3), 3);
    }
}
