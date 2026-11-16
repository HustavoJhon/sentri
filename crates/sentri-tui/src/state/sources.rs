//! Sources section: list, add, edit, enable/disable, delete and refresh.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use sentri_app::SourceConfig;
use sentri_app::settings::{CUSTOM_KINDS, SourceDraft};

use crate::input::LineInput;

/// Rows of the source form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormRow {
    Name,
    Kind,
    Url,
    Content,
    Category,
    Enabled,
}

pub const FORM_ROWS: [FormRow; 6] = [
    FormRow::Name,
    FormRow::Kind,
    FormRow::Url,
    FormRow::Content,
    FormRow::Category,
    FormRow::Enabled,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceForm {
    /// Name of the source being edited, `None` when adding.
    pub original: Option<String>,
    pub draft: SourceDraft,
    pub row: usize,
    pub editing: Option<LineInput>,
    pub error: Option<String>,
}

impl SourceForm {
    fn new_source() -> Self {
        Self {
            original: None,
            draft: SourceDraft {
                kind: "rss".into(),
                content: "news".into(),
                enabled: true,
                ..SourceDraft::default()
            },
            row: 0,
            editing: None,
            error: None,
        }
    }

    fn edit(source: &SourceConfig) -> Self {
        Self {
            original: Some(source.name.clone()),
            draft: SourceDraft {
                name: source.name.clone(),
                kind: source.kind.as_str().into(),
                url: source.url.clone().unwrap_or_default(),
                content: if source.content == sentri_core::ContentKind::News {
                    "news".into()
                } else {
                    "opportunity".into()
                },
                category: source
                    .category
                    .map(|c| c.as_str().to_owned())
                    .unwrap_or_default(),
                enabled: source.enabled,
            },
            row: 0,
            editing: None,
            error: None,
        }
    }

    pub fn current(&self) -> FormRow {
        FORM_ROWS[self.row.min(FORM_ROWS.len() - 1)]
    }

    fn validate(&self) -> Result<(), String> {
        let name = self.draft.name.trim();
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        {
            return Err("Name: letters, digits, '-', '_' or '.'".into());
        }
        let url = self.draft.url.trim();
        if !(url.starts_with("https://")
            || url.starts_with("http://")
            || url.starts_with("webcal://"))
        {
            return Err("URL must start with https://, http:// or webcal://".into());
        }
        if !self.draft.category.trim().is_empty()
            && self
                .draft
                .category
                .parse::<sentri_core::Category>()
                .is_err()
        {
            return Err("Category: event, course, hackathon, job… or empty".into());
        }
        Ok(())
    }

    fn cycle(&mut self, forward: bool) {
        match self.current() {
            FormRow::Kind => {
                let i = CUSTOM_KINDS
                    .iter()
                    .position(|k| *k == self.draft.kind)
                    .unwrap_or(0);
                let n = CUSTOM_KINDS.len();
                self.draft.kind = CUSTOM_KINDS[if forward {
                    (i + 1) % n
                } else {
                    (i + n - 1) % n
                }]
                .into();
            }
            FormRow::Content => {
                self.draft.content = if self.draft.content == "news" {
                    "opportunity"
                } else {
                    "news"
                }
                .into();
            }
            FormRow::Enabled => self.draft.enabled = !self.draft.enabled,
            _ => {}
        }
    }

    fn text_mut(&mut self) -> Option<&mut String> {
        match self.current() {
            FormRow::Name => Some(&mut self.draft.name),
            FormRow::Url => Some(&mut self.draft.url),
            FormRow::Category => Some(&mut self.draft.category),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcesEffect {
    Save {
        original: Option<String>,
        draft: SourceDraft,
    },
    Delete(String),
    Toggle {
        name: String,
        enabled: bool,
    },
    Refresh(String),
    Message(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourcesState {
    pub selected: usize,
    pub form: Option<SourceForm>,
    pub confirm_delete: Option<String>,
}

impl SourcesState {
    fn on_form_key(&mut self, mut form: SourceForm, key: KeyEvent) -> Vec<SourcesEffect> {
        if let Some(mut input) = form.editing.take() {
            match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter => {
                    if let Some(text) = form.text_mut() {
                        input.value().trim().clone_into(text);
                    }
                }
                _ => {
                    input.handle(key);
                    form.editing = Some(input);
                }
            }
            self.form = Some(form);
            return Vec::new();
        }
        let last = FORM_ROWS.len() - 1;
        match key.code {
            KeyCode::Esc => return Vec::new(),
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Err(e) = form.validate() {
                    form.error = Some(e);
                } else {
                    return vec![SourcesEffect::Save {
                        original: form.original,
                        draft: form.draft,
                    }];
                }
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                form.row = (form.row + 1).min(last);
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                form.row = form.row.saturating_sub(1);
            }
            KeyCode::Left => form.cycle(false),
            KeyCode::Right => form.cycle(true),
            KeyCode::Enter => {
                if let Some(text) = form.text_mut() {
                    let value = text.clone();
                    form.editing = Some(LineInput::with_value(&value));
                } else {
                    form.cycle(true);
                }
            }
            _ => {}
        }
        self.form = Some(form);
        Vec::new()
    }

    /// Handles keys in the Sources section. `None` lets global keys apply.
    pub fn on_key(
        &mut self,
        key: KeyEvent,
        sources: &[SourceConfig],
    ) -> Option<Vec<SourcesEffect>> {
        if let Some(form) = self.form.take() {
            return Some(self.on_form_key(form, key));
        }
        if let Some(name) = self.confirm_delete.take() {
            return Some(if matches!(key.code, KeyCode::Char('y' | 'Y')) {
                vec![SourcesEffect::Delete(name)]
            } else {
                Vec::new()
            });
        }
        let last = sources.len().saturating_sub(1);
        let selected = sources.get(self.selected.min(last));
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => self.selected = (self.selected + 1).min(last),
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('+') => self.form = Some(SourceForm::new_source()),
            KeyCode::Char('e') | KeyCode::Enter => {
                let source = selected?;
                if CUSTOM_KINDS.contains(&source.kind.as_str()) {
                    self.form = Some(SourceForm::edit(source));
                } else {
                    return Some(vec![SourcesEffect::Message(format!(
                        "{} sources are edited in config.toml (they need queries or credentials)",
                        source.kind.label()
                    ))]);
                }
            }
            KeyCode::Char(' ' | 't') => {
                let source = selected?;
                return Some(vec![SourcesEffect::Toggle {
                    name: source.name.clone(),
                    enabled: !source.enabled,
                }]);
            }
            KeyCode::Char('x') => self.confirm_delete = Some(selected?.name.clone()),
            KeyCode::Char('u') => {
                return Some(vec![SourcesEffect::Refresh(selected?.name.clone())]);
            }
            _ => return None,
        }
        Some(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn sources() -> Vec<SourceConfig> {
        sentri_app::Config::parse(sentri_app::config::DEFAULT_CONFIG, Path::new("d"))
            .unwrap()
            .sources
    }

    fn typed(s: &mut SourcesState, srcs: &[SourceConfig], text: &str) {
        for c in text.chars() {
            s.on_key(key(KeyCode::Char(c)), srcs);
        }
    }

    #[test]
    fn adds_a_feed_through_the_form() {
        let srcs = sources();
        let mut s = SourcesState::default();
        s.on_key(key(KeyCode::Char('+')), &srcs);
        s.on_key(key(KeyCode::Enter), &srcs);
        typed(&mut s, &srcs, "my-blog");
        s.on_key(key(KeyCode::Enter), &srcs);
        s.on_key(key(KeyCode::Down), &srcs);
        s.on_key(key(KeyCode::Down), &srcs);
        s.on_key(key(KeyCode::Enter), &srcs);
        typed(&mut s, &srcs, "https://blog.example/feed");
        s.on_key(key(KeyCode::Enter), &srcs);
        let effects = s
            .on_key(
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
                &srcs,
            )
            .unwrap();
        let [
            SourcesEffect::Save {
                original: None,
                draft,
            },
        ] = effects.as_slice()
        else {
            panic!("expected save: {effects:?}");
        };
        assert_eq!(draft.name, "my-blog");
        assert_eq!(draft.kind, "rss");
        assert_eq!(draft.content, "news");
    }

    #[test]
    fn validates_before_saving() {
        let srcs = sources();
        let mut s = SourcesState::default();
        s.on_key(key(KeyCode::Char('+')), &srcs);
        s.on_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &srcs,
        );
        assert!(s.form.as_ref().unwrap().error.is_some());
    }

    #[test]
    fn toggles_refreshes_and_confirms_deletes() {
        let srcs = sources();
        let mut s = SourcesState::default();
        let first = srcs[0].name.clone();
        assert_eq!(
            s.on_key(key(KeyCode::Char(' ')), &srcs),
            Some(vec![SourcesEffect::Toggle {
                name: first.clone(),
                enabled: !srcs[0].enabled
            }])
        );
        assert_eq!(
            s.on_key(key(KeyCode::Char('u')), &srcs),
            Some(vec![SourcesEffect::Refresh(first.clone())])
        );
        s.on_key(key(KeyCode::Char('x')), &srcs);
        assert_eq!(s.on_key(key(KeyCode::Char('n')), &srcs), Some(vec![]));
        s.on_key(key(KeyCode::Char('x')), &srcs);
        assert_eq!(
            s.on_key(key(KeyCode::Char('y')), &srcs),
            Some(vec![SourcesEffect::Delete(first)])
        );
    }

    #[test]
    fn non_feed_sources_are_not_edited_in_the_form() {
        let srcs = sources();
        let mut s = SourcesState {
            selected: srcs
                .iter()
                .position(|x| x.kind == sentri_core::SourceKind::GitHub)
                .unwrap(),
            ..SourcesState::default()
        };
        assert!(matches!(
            s.on_key(key(KeyCode::Char('e')), &srcs).unwrap().as_slice(),
            [SourcesEffect::Message(_)]
        ));
        assert!(s.form.is_none());
    }
}
