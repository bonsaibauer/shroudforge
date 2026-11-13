pub(crate) const SERVICE: &str = "shroudforge.notifications";
pub(crate) fn persist_then_mirror<E>(
    persist: impl FnOnce() -> Result<(), E>,
    mirror: impl FnOnce() -> i32,
) -> Result<(), E> {
    persist()?;
    let _ = mirror();
    Ok(())
}
fn identifier(value: &str, bound: usize) -> bool {
    !value.is_empty()
        && value.len() <= bound
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}
pub(crate) fn level_name(level: u32) -> Option<&'static str> {
    match level {
        0 => Some("info"),
        1 => Some("success"),
        2 => Some("warning"),
        3 => Some("error"),
        4 => Some("update"),
        _ => None,
    }
}
// The persisted feed uses severity names, while the optional presenter also
// distinguishes success/update styling. Do not change the feed schema.
pub(crate) fn feed_level_name(level: u32) -> Option<&'static str> {
    match level {
        0 | 1 | 4 => Some("info"),
        2 => Some("warn"),
        3 => Some("error"),
        _ => None,
    }
}
pub(crate) fn valid(package: &str, id: &str, title: &str, message: &str, level: u32) -> bool {
    identifier(package, 256)
        && identifier(id, 80)
        && package.len() + id.len() + 1 <= 200
        && !title.is_empty()
        && title.len() <= 120
        && !title.contains('\0')
        && !message.is_empty()
        && message.len() <= 2000
        && !message.contains('\0')
        && level_name(level).is_some()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notice_protocol_bounds_and_namespace_are_exact() {
        assert!(valid(
            "minimap_mod",
            "visibility",
            "Minimap",
            "Minimap On",
            0
        ));
        for (p, id, t, m, l) in [
            ("", "id", "t", "m", 0),
            ("other/pkg", "id", "t", "m", 0),
            ("pkg", "spoof/id", "t", "m", 0),
            ("pkg", "id", "t\0", "m", 0),
            ("pkg", "id", "t", "m", 5),
        ] {
            assert!(!valid(p, id, t, m, l));
        }
        assert!(!valid("pkg", &"a".repeat(81), "t", "m", 0));
        assert!(!valid(&"a".repeat(200), &"b".repeat(40), "t", "m", 0));
        assert!(!valid("pkg", "id", &"é".repeat(61), "m", 0));
        assert!(!valid("pkg", "id", "t", &"a".repeat(2001), 0));
        assert_eq!(level_name(2), Some("warning"));
        assert_eq!(
            (0..5).map(feed_level_name).collect::<Vec<_>>(),
            vec![
                Some("info"),
                Some("info"),
                Some("warn"),
                Some("error"),
                Some("info")
            ]
        );
        assert!(valid(&"a".repeat(159), &"b".repeat(40), "t", "m", 0));
        assert!(!valid(&"a".repeat(160), &"b".repeat(40), "t", "m", 0));
    }
    #[test]
    fn notice_protocol_persistence_is_authoritative() {
        assert_eq!(
            persist_then_mirror(|| Err("disk"), || panic!("mirror before persist")),
            Err("disk")
        );
        for status in 0..=10 {
            let stored = std::cell::Cell::new(false);
            assert_eq!(
                persist_then_mirror(
                    || {
                        stored.set(true);
                        Ok::<_, ()>(())
                    },
                    || {
                        assert!(stored.get());
                        status
                    }
                ),
                Ok(())
            );
        }
    }
}
