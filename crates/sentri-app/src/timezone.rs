//! IANA time zone names: detection from the system and format checks.
//!
//! SENTRI does not bundle the time zone database; a name is accepted when it
//! has the `Area/Location` shape (or is `UTC`), and the system's own zone is
//! offered as the default.

use std::path::Path;

/// Common zones offered for quick selection, west to east.
pub const SUGGESTIONS: &[&str] = &[
    "America/Los_Angeles",
    "America/Denver",
    "America/Chicago",
    "America/Mexico_City",
    "America/New_York",
    "America/Bogota",
    "America/Lima",
    "America/Guayaquil",
    "America/Caracas",
    "America/La_Paz",
    "America/Santiago",
    "America/Asuncion",
    "America/Argentina/Buenos_Aires",
    "America/Montevideo",
    "America/Sao_Paulo",
    "UTC",
    "Europe/London",
    "Europe/Lisbon",
    "Europe/Madrid",
    "Europe/Berlin",
    "Africa/Lagos",
    "Africa/Nairobi",
    "Asia/Dubai",
    "Asia/Kolkata",
    "Asia/Singapore",
    "Asia/Tokyo",
    "Australia/Sydney",
];

/// Whether `name` looks like an IANA zone (`America/Lima`, `UTC`).
pub fn valid(name: &str) -> bool {
    if matches!(name, "UTC" | "Etc/UTC") {
        return true;
    }
    let mut parts = name.split('/');
    let Some(area) = parts.next() else {
        return false;
    };
    let rest: Vec<&str> = parts.collect();
    let part_ok = |p: &str| {
        !p.is_empty()
            && p.len() <= 30
            && p.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+'))
    };
    !rest.is_empty() && rest.len() <= 2 && part_ok(area) && rest.iter().all(|p| part_ok(p))
}

/// The system time zone: `TZ`, then the `/etc/localtime` link target.
pub fn detect() -> Option<String> {
    detect_from(
        std::env::var("TZ").ok().as_deref(),
        Path::new("/etc/localtime"),
    )
}

fn detect_from(tz: Option<&str>, localtime: &Path) -> Option<String> {
    let from_env = tz
        .map(|t| t.trim().trim_start_matches(':'))
        .filter(|t| valid(t))
        .map(str::to_owned);
    from_env.or_else(|| {
        let target = std::fs::read_link(localtime).ok()?;
        let text = target.to_string_lossy();
        let (_, zone) = text.split_once("zoneinfo/")?;
        valid(zone).then(|| zone.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_iana_shapes_only() {
        for ok in [
            "America/Lima",
            "UTC",
            "America/Argentina/Buenos_Aires",
            "Etc/GMT+5",
        ] {
            assert!(valid(ok), "{ok}");
        }
        for bad in [
            "",
            "Lima",
            "America/",
            "/Lima",
            "America/Li ma",
            "../etc/passwd",
            "a/b/c/d",
        ] {
            assert!(!valid(bad), "{bad}");
        }
        assert!(SUGGESTIONS.iter().all(|z| valid(z)));
    }

    #[cfg(unix)]
    #[test]
    fn detects_from_tz_then_localtime_link() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("localtime");
        std::os::unix::fs::symlink("/usr/share/zoneinfo/America/Lima", &link).unwrap();
        assert_eq!(
            detect_from(Some(":Europe/Madrid"), &link).as_deref(),
            Some("Europe/Madrid")
        );
        assert_eq!(
            detect_from(Some("garbage"), &link).as_deref(),
            Some("America/Lima")
        );
        assert_eq!(detect_from(None, &dir.path().join("missing")), None);
    }
}
