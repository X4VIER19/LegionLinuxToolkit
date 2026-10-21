//! Embedded data for hardware-isolated tests and demos.

pub const VALID: &str = include_str!("../data/valid.snapshot");
pub const MISSING: &str = include_str!("../data/missing.snapshot");
pub const MALFORMED: &str = include_str!("../data/malformed.snapshot");

/// Only named embedded fixtures are available; host paths are never opened.
pub fn named(name: &str) -> Option<&'static str> {
    match name {
        "valid" => Some(VALID),
        "missing" => Some(MISSING),
        "malformed" => Some(MALFORMED),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_host_path_names() {
        assert!(named("/sys").is_none());
        assert!(named("/proc").is_none());
        assert!(named("/dev").is_none());
    }
}
