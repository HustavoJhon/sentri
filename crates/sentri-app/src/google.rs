//! Google Calendar setup diagnostics.
//!
//! The OAuth flow and the sync itself are not implemented yet, so this module
//! never reports a working connection. It checks what the user can prepare
//! today (credentials file, permissions, format) and lists the next steps.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// One check of the diagnosis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub ok: bool,
    pub label: String,
    pub detail: String,
}

/// Result of [`diagnose`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnosis {
    pub credentials_path: PathBuf,
    pub checks: Vec<Check>,
    /// Always `false` in this version: synchronization is not implemented.
    pub connected: bool,
    pub next_steps: Vec<String>,
}

/// Default location of the OAuth client file.
pub fn default_credentials_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "sentri").map_or_else(
        || PathBuf::from("google-oauth.json"),
        |d| d.config_dir().join("google-oauth.json"),
    )
}

fn check(ok: bool, label: &str, detail: impl Into<String>) -> Check {
    Check {
        ok,
        label: label.into(),
        detail: detail.into(),
    }
}

/// Inspects the credentials file without ever printing secrets.
pub fn diagnose(credentials: &Path) -> Diagnosis {
    let mut checks = Vec::new();
    let mut next = Vec::new();
    let exists = credentials.is_file();
    checks.push(check(
        exists,
        "OAuth client file",
        if exists {
            credentials.display().to_string()
        } else {
            format!("missing: {}", credentials.display())
        },
    ));
    if exists {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(credentials).map_or(0, |m| m.permissions().mode() & 0o777);
            // No permission bits for group or others.
            let group_or_others = 0o077;
            let private = mode & group_or_others == 0;
            checks.push(check(
                private,
                "Private permissions",
                format!("{mode:o} (expected 600)"),
            ));
            if !private {
                next.push(format!("chmod 600 {}", credentials.display()));
            }
        }
        let parsed = std::fs::read_to_string(credentials)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok());
        let installed = parsed.as_ref().and_then(|v| v.get("installed"));
        let has_client = installed
            .and_then(|i| i.get("client_id"))
            .and_then(Value::as_str)
            .is_some_and(|id| id.ends_with(".apps.googleusercontent.com"));
        let has_secret = installed.and_then(|i| i.get("client_secret")).is_some();
        checks.push(check(
            parsed.is_some(),
            "Valid JSON",
            if parsed.is_some() {
                "yes"
            } else {
                "the file is not valid JSON"
            },
        ));
        checks.push(check(
            has_client && has_secret,
            "Desktop app client",
            if has_client && has_secret {
                "client id and secret present (not shown)"
            } else {
                "expected an \"installed\" (Desktop app) OAuth client"
            },
        ));
        if !(has_client && has_secret) {
            next.push(
                "Create OAuth credentials of type \"Desktop app\" and download the JSON.".into(),
            );
        }
    } else {
        next.push(
            "Google Cloud Console: create a project and enable the Google Calendar API.".into(),
        );
        next.push("Create OAuth credentials of type \"Desktop app\" and download the JSON.".into());
        next.push(format!(
            "Save it as {} with permissions 600.",
            credentials.display()
        ));
    }
    checks.push(check(
        false,
        "Authorization and sync",
        "not implemented in this version: nothing is sent to Google",
    ));
    next.push("Once sync is implemented, SENTRI will create its own \"SENTRI Opportunities\" calendar and never touch personal events.".into());
    Diagnosis {
        credentials_path: credentials.to_path_buf(),
        checks,
        connected: false,
        next_steps: next,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_credentials_explain_the_setup() {
        let d = diagnose(Path::new("/nonexistent/google-oauth.json"));
        assert!(!d.connected);
        assert!(!d.checks[0].ok);
        assert!(d.next_steps.iter().any(|s| s.contains("Desktop app")));
    }

    #[test]
    fn valid_credentials_are_checked_but_never_reported_as_connected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("google-oauth.json");
        std::fs::write(
            &path,
            r#"{"installed":{"client_id":"123.apps.googleusercontent.com","client_secret":"s3cret"}}"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        let d = diagnose(&path);
        assert!(!d.connected);
        let text = format!("{d:?}");
        assert!(!text.contains("s3cret"), "secrets are never exposed");
        assert!(
            d.checks
                .iter()
                .any(|c| c.label == "Desktop app client" && c.ok)
        );
        #[cfg(unix)]
        assert!(d.next_steps.iter().any(|s| s.starts_with("chmod 600")));
    }
}
