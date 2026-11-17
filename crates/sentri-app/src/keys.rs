//! Configurable keyboard shortcuts (`[keys]` in `config.toml`).
//!
//! Actions map to key specs such as `l`, `S`, `ctrl-p`, `enter` or `f5`.
//! Navigation keys (j/k, arrows, Tab, digits, Enter, Esc…) are fixed and
//! cannot be assigned, so the interface always stays usable.

use std::collections::BTreeMap;
use std::fmt;

/// A key with modifiers, independent of the terminal library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeySpec {
    pub ctrl: bool,
    pub alt: bool,
    pub key: KeyName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeyName {
    Char(char),
    Enter,
    Esc,
    Tab,
    Backspace,
    Space,
    F(u8),
}

impl fmt::Display for KeySpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            f.write_str("ctrl-")?;
        }
        if self.alt {
            f.write_str("alt-")?;
        }
        match self.key {
            KeyName::Char(c) => write!(f, "{c}"),
            KeyName::Enter => f.write_str("enter"),
            KeyName::Esc => f.write_str("esc"),
            KeyName::Tab => f.write_str("tab"),
            KeyName::Backspace => f.write_str("backspace"),
            KeyName::Space => f.write_str("space"),
            KeyName::F(n) => write!(f, "f{n}"),
        }
    }
}

/// Parses `l`, `S`, `?`, `ctrl-p`, `alt-x`, `space`, `f5`.
pub fn parse_key(text: &str) -> Result<KeySpec, String> {
    let mut rest = text.trim();
    let (mut ctrl, mut alt) = (false, false);
    loop {
        let lower = rest.to_ascii_lowercase();
        if let Some(r) = lower
            .strip_prefix("ctrl-")
            .or_else(|| lower.strip_prefix("c-"))
        {
            ctrl = true;
            rest = &rest[rest.len() - r.len()..];
        } else if let Some(r) = lower
            .strip_prefix("alt-")
            .or_else(|| lower.strip_prefix("m-"))
        {
            alt = true;
            rest = &rest[rest.len() - r.len()..];
        } else {
            break;
        }
    }
    let lower = rest.to_ascii_lowercase();
    let key = match lower.as_str() {
        "enter" | "return" => KeyName::Enter,
        "esc" | "escape" => KeyName::Esc,
        "tab" => KeyName::Tab,
        "backspace" => KeyName::Backspace,
        "space" => KeyName::Space,
        f if f.len() > 1
            && f.starts_with('f')
            && f[1..].parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)) =>
        {
            KeyName::F(f[1..].parse().unwrap_or(1))
        }
        _ => {
            let mut chars = rest.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if !c.is_control() && !c.is_whitespace() => {
                    // Ctrl combinations are case-insensitive in terminals.
                    KeyName::Char(if ctrl { c.to_ascii_lowercase() } else { c })
                }
                _ => return Err(format!("unknown key {text:?}; use e.g. l, S, ctrl-p, f5")),
            }
        }
    };
    Ok(KeySpec { ctrl, alt, key })
}

macro_rules! actions {
    ($($variant:ident => $name:literal, $default:literal, $label:literal;)*) => {
        /// A remappable action.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum KeyAction { $($variant,)* }

        impl KeyAction {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            pub const fn name(self) -> &'static str {
                match self { $(Self::$variant => $name,)* }
            }

            pub const fn default_key(self) -> &'static str {
                match self { $(Self::$variant => $default,)* }
            }

            pub const fn label(self) -> &'static str {
                match self { $(Self::$variant => $label,)* }
            }
        }
    };
}

actions! {
    Like => "like", "l", "Like";
    Save => "save", "s", "Save";
    Dismiss => "dismiss", "d", "Dismiss";
    Interesting => "interesting", "i", "Mark interesting";
    Applied => "applied", "p", "Mark applied";
    Enrolled => "enrolled", "e", "Mark enrolled";
    Completed => "completed", "c", "Mark completed";
    Schedule => "schedule", "a", "Add to calendar";
    Edit => "edit", "E", "Edit item";
    Open => "open", "o", "Open link";
    AddLink => "add_link", "n", "Add link";
    Search => "search", "/", "Search";
    Palette => "palette", ":", "Command palette";
    Density => "density", "v", "Card density";
    Sort => "sort", "S", "Cycle sort";
    Collect => "collect", "R", "Collect now";
    Reload => "reload", "r", "Reload";
    Help => "help", "?", "Help";
    Quit => "quit", "q", "Quit";
    FilterFree => "filter_free", "F", "Filter: free";
    FilterTravel => "filter_travel", "T", "Filter: travel";
    FilterUpcoming => "filter_upcoming", "U", "Filter: upcoming";
    FilterPast => "filter_past", "H", "Filter: past";
    FilterEligible => "filter_eligible", "P", "Filter: open to my country";
    FilterCertificate => "filter_certificate", "C", "Filter: certificate";
    FilterModality => "filter_modality", "M", "Filter: modality";
    FilterCity => "filter_city", "L", "Filter: city";
}

impl KeyAction {
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|a| a.name() == name)
    }
}

/// Keys reserved for navigation and editing; they cannot be remapped.
pub fn is_reserved(spec: KeySpec) -> bool {
    if spec.ctrl {
        return matches!(spec.key, KeyName::Char('c' | 's'));
    }
    if spec.alt {
        return false;
    }
    match spec.key {
        // Section-specific keys (calendar h/l, sources +/x…) are not
        // reserved: inside those sections they take precedence.
        KeyName::Char(c) => "jkgG[]0123456789".contains(c),
        KeyName::Enter | KeyName::Esc | KeyName::Tab | KeyName::Backspace | KeyName::Space => true,
        KeyName::F(_) => false,
    }
}

/// Effective key of every action: defaults overridden by `[keys]`.
pub fn effective(
    config: &BTreeMap<String, String>,
) -> Result<BTreeMap<KeyAction, KeySpec>, Vec<String>> {
    let mut errors = Vec::new();
    for name in config.keys() {
        if KeyAction::from_name(name).is_none() {
            errors.push(format!("unknown action {name:?}"));
        }
    }
    let mut map = BTreeMap::new();
    for action in KeyAction::ALL {
        let text = config
            .get(action.name())
            .map_or(action.default_key(), String::as_str);
        match parse_key(text) {
            Ok(spec) if is_reserved(spec) => errors.push(format!(
                "{}: {spec} is reserved for navigation",
                action.name()
            )),
            Ok(spec) => {
                map.insert(*action, spec);
            }
            Err(e) => errors.push(format!("{}: {e}", action.name())),
        }
    }
    // Two actions on the same key.
    let mut seen: BTreeMap<KeySpec, KeyAction> = BTreeMap::new();
    for (action, spec) in &map {
        if let Some(other) = seen.insert(*spec, *action) {
            errors.push(format!(
                "{spec} is assigned to both {} and {}",
                other.name(),
                action.name()
            ));
        }
    }
    if errors.is_empty() {
        Ok(map)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect()
    }

    #[test]
    fn parses_key_specs() {
        assert_eq!(parse_key("l").unwrap().to_string(), "l");
        assert_eq!(parse_key("S").unwrap().key, KeyName::Char('S'));
        assert_eq!(parse_key("Ctrl-P").unwrap().to_string(), "ctrl-p");
        assert_eq!(parse_key("f5").unwrap().key, KeyName::F(5));
        assert_eq!(parse_key("alt-x").unwrap().to_string(), "alt-x");
        assert!(parse_key("ll").is_err());
        assert!(parse_key("f13").is_err());
        assert!(parse_key("").is_err());
    }

    #[test]
    fn defaults_are_valid_and_unique() {
        let map = effective(&BTreeMap::new()).unwrap();
        assert_eq!(map.len(), KeyAction::ALL.len());
    }

    #[test]
    fn detects_conflicts_reserved_and_unknown() {
        let errors = effective(&keys(&[("like", "s")])).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("both")), "{errors:?}");
        let errors = effective(&keys(&[("save", "j")])).unwrap_err();
        assert!(errors[0].contains("reserved"));
        let errors = effective(&keys(&[("teleport", "z")])).unwrap_err();
        assert!(errors[0].contains("unknown action"));
        // Swapping two keys is fine.
        assert!(effective(&keys(&[("like", "s"), ("save", "l")])).is_ok());
    }
}
