use std::sync::OnceLock;
use std::time::Instant;

/// Seconds since the first call in this process, from a monotonic clock, so a
/// mod can measure how long its own work takes: the Lua sandbox has no `os`.
pub(crate) fn read_monotonic_clock() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_seconds_forward() {
        let first = read_monotonic_clock();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let second = read_monotonic_clock();
        assert!(first >= 0.0);
        assert!(second - first >= 0.015, "{first} then {second}");
        assert!(second - first < 5.0, "{first} then {second}");
    }
}
