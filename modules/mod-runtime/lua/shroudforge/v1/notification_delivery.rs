// Feed persistence is authoritative. The optional mirror cannot change its result.
pub(super) fn deliver<E>(
    persist: impl FnOnce() -> Result<(), E>,
    mirror: impl FnOnce() -> i32,
) -> Result<(), E> {
    persist()?;
    let _ = mirror();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_mirror_before_successful_persistence() {
        assert_eq!(
            deliver(|| Err("disk"), || panic!("mirror called")),
            Err("disk")
        );
    }
    #[test]
    fn all_mirror_statuses_preserve_feed_success_and_order() {
        for status in 0..=10 {
            let persisted = std::cell::Cell::new(false);
            assert_eq!(
                deliver(
                    || {
                        persisted.set(true);
                        Ok::<_, ()>(())
                    },
                    || {
                        assert!(persisted.get());
                        status
                    }
                ),
                Ok(())
            );
        }
    }
}
