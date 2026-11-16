//! Settings form: edit, validate, save with backup, restore defaults.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use sentri_app::Config;
use sentri_app::settings::{SETTINGS, SettingKind, current, default_value, source_settings};

use crate::input::LineInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingRow {
    pub key: String,
    pub group: String,
    pub label: String,
    pub kind: SettingKind,
    pub help: String,
    pub live: bool,
    pub value: String,
    pub original: String,
}

impl SettingRow {
    pub fn changed(&self) -> bool {
        self.value != self.original
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsEffect {
    Save(Vec<(String, String)>),
    RestoreDefaults,
    Message(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsState {
    pub rows: Vec<SettingRow>,
    pub selected: usize,
    pub editing: Option<LineInput>,
    pub error: Option<String>,
    pub confirm_restore: bool,
}

impl SettingsState {
    pub fn from_config(config: &Config) -> Self {
        let mut rows: Vec<SettingRow> = SETTINGS
            .iter()
            .map(|s| {
                let value = current(config, s.key).unwrap_or_default();
                SettingRow {
                    key: s.key.into(),
                    group: s.group.into(),
                    label: s.label.into(),
                    kind: s.kind,
                    help: s.help.into(),
                    live: s.live,
                    original: value.clone(),
                    value,
                }
            })
            .collect();
        for (key, label, kind) in source_settings(config) {
            let value = current(config, &key).unwrap_or_default();
            rows.push(SettingRow {
                key,
                group: "Sources".into(),
                label,
                kind,
                help: "Each source is independent".into(),
                live: false,
                original: value.clone(),
                value,
            });
        }
        Self {
            rows,
            ..Self::default()
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.rows.iter().any(SettingRow::changed)
    }

    fn row(&mut self) -> Option<&mut SettingRow> {
        self.rows.get_mut(self.selected)
    }

    fn step(&mut self, forward: bool) {
        let Some(row) = self.row() else {
            return;
        };
        row.value = match row.kind {
            SettingKind::Bool => (row.value != "true").to_string(),
            SettingKind::Choice(options) => {
                let i = options.iter().position(|o| *o == row.value).unwrap_or(0);
                let n = options.len();
                options[if forward {
                    (i + 1) % n
                } else {
                    (i + n - 1) % n
                }]
                .to_owned()
            }
            SettingKind::Int { min, max } => {
                let v: i64 = row.value.parse().unwrap_or(min);
                (v + if forward { 1 } else { -1 })
                    .clamp(min, max)
                    .to_string()
            }
            SettingKind::Text => return,
        };
    }

    fn validate_text(kind: SettingKind, text: &str) -> Result<String, String> {
        match kind {
            SettingKind::Int { min, max } => {
                let n: i64 = text.trim().parse().map_err(|_| "must be a whole number")?;
                if (min..=max).contains(&n) {
                    Ok(n.to_string())
                } else {
                    Err(format!("between {min} and {max}"))
                }
            }
            _ => Ok(text.trim().to_owned()),
        }
    }

    /// Handles keys in the Settings section; `None` lets global keys apply.
    pub fn on_key(&mut self, key: KeyEvent) -> Option<Vec<SettingsEffect>> {
        if self.confirm_restore {
            self.confirm_restore = false;
            return Some(if matches!(key.code, KeyCode::Char('y' | 'Y')) {
                vec![SettingsEffect::RestoreDefaults]
            } else {
                Vec::new()
            });
        }
        if let Some(mut input) = self.editing.take() {
            match key.code {
                KeyCode::Esc => self.error = None,
                KeyCode::Enter => {
                    let kind = self.rows.get(self.selected).map(|r| r.kind)?;
                    match Self::validate_text(kind, &input.value()) {
                        Ok(v) => {
                            self.error = None;
                            if let Some(row) = self.row() {
                                row.value = v;
                            }
                        }
                        Err(e) => {
                            self.error = Some(e);
                            self.editing = Some(input);
                        }
                    }
                }
                _ => {
                    input.handle(key);
                    self.editing = Some(input);
                }
            }
            return Some(Vec::new());
        }
        let last = self.rows.len().saturating_sub(1);
        match key.code {
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let changes: Vec<(String, String)> = self
                    .rows
                    .iter()
                    .filter(|r| r.changed())
                    .map(|r| (r.key.clone(), r.value.clone()))
                    .collect();
                if changes.is_empty() {
                    return Some(vec![SettingsEffect::Message("No changes to save.".into())]);
                }
                return Some(vec![SettingsEffect::Save(changes)]);
            }
            KeyCode::Down | KeyCode::Char('j') => self.selected = (self.selected + 1).min(last),
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::PageDown => self.selected = (self.selected + 10).min(last),
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(10),
            KeyCode::Left | KeyCode::Char('h') => self.step(false),
            KeyCode::Right | KeyCode::Char('l') => self.step(true),
            KeyCode::Enter => {
                let row = self.rows.get(self.selected)?;
                if matches!(row.kind, SettingKind::Text | SettingKind::Int { .. }) {
                    self.editing = Some(LineInput::with_value(&row.value));
                } else {
                    self.step(true);
                }
            }
            KeyCode::Char('r') => {
                let row = self.row()?;
                row.value = default_value(&row.key).unwrap_or_else(|| row.original.clone());
            }
            KeyCode::Char('u') => {
                for row in &mut self.rows {
                    row.value = row.original.clone();
                }
            }
            KeyCode::Char('D') => self.confirm_restore = true,
            _ => return None,
        }
        Some(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn state() -> SettingsState {
        let config = Config::parse(sentri_app::config::DEFAULT_CONFIG, Path::new("d")).unwrap();
        SettingsState::from_config(&config)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn select(s: &mut SettingsState, key_name: &str) {
        s.selected = s.rows.iter().position(|r| r.key == key_name).unwrap();
    }

    #[test]
    fn edits_and_saves_only_changes() {
        let mut s = state();
        select(&mut s, "ui.density");
        s.on_key(key(KeyCode::Right));
        select(&mut s, "ui.show_sidebar");
        s.on_key(key(KeyCode::Enter));
        select(&mut s, "github.username");
        s.on_key(key(KeyCode::Enter));
        for c in "octocat".chars() {
            s.on_key(key(KeyCode::Char(c)));
        }
        s.on_key(key(KeyCode::Enter));
        let effects = s
            .on_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))
            .unwrap();
        assert_eq!(
            effects,
            vec![SettingsEffect::Save(vec![
                ("ui.density".into(), "detailed".into()),
                ("ui.show_sidebar".into(), "false".into()),
                ("github.username".into(), "octocat".into()),
            ])]
        );
    }

    #[test]
    fn validates_numbers_and_restores_defaults() {
        let mut s = state();
        select(&mut s, "ui.feed_width");
        s.on_key(key(KeyCode::Enter));
        s.editing = Some(LineInput::with_value("200"));
        s.on_key(key(KeyCode::Enter));
        assert!(s.error.is_some());
        s.on_key(key(KeyCode::Esc));
        s.on_key(key(KeyCode::Right));
        assert_eq!(s.rows[s.selected].value, "56");
        s.on_key(key(KeyCode::Char('r')));
        assert_eq!(s.rows[s.selected].value, "55");
        s.on_key(key(KeyCode::Char('D')));
        assert_eq!(
            s.on_key(key(KeyCode::Char('y'))),
            Some(vec![SettingsEffect::RestoreDefaults])
        );
    }

    #[test]
    fn includes_every_source() {
        let s = state();
        assert!(
            s.rows
                .iter()
                .any(|r| r.key == "sources.hn-front-page.enabled")
        );
    }
}
