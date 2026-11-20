//! Opportunity editor: every field, personal notes and custom fields.
//!
//! Values are validated with the same rules as storage before they can be
//! saved. Saving stores only changed fields as manual overrides; "reset"
//! restores the detected value.

use std::collections::BTreeSet;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use sentri_app::EditState;
use sentri_core::{Annotations, Field, FieldKind, Opportunity, OpportunityId, Priority};

use crate::input::LineInput;

/// A row of the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Field(Field),
    Notes,
    Priority,
    Custom(usize),
    AddCustom,
}

/// What the editor asks the runtime to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorOutcome {
    Continue,
    Close,
    Save {
        id: OpportunityId,
        values: Vec<(Field, String)>,
        reset: Vec<Field>,
        annotations: Annotations,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorState {
    pub id: OpportunityId,
    pub detected: Opportunity,
    pub effective: Opportunity,
    /// Current text per field, indexed like `Field::ALL`.
    pub values: Vec<String>,
    /// Fields stored as manual overrides when the editor opened.
    pub overridden: BTreeSet<Field>,
    pub reset: BTreeSet<Field>,
    pub notes: String,
    pub priority: Option<Priority>,
    pub custom: Vec<(String, String)>,
    pub annotations_at_open: Annotations,
    pub row: usize,
    pub editing: Option<LineInput>,
    pub error: Option<String>,
    pub confirm_discard: bool,
}

fn field_index(field: Field) -> usize {
    Field::ALL.iter().position(|f| *f == field).unwrap_or(0)
}

impl EditorState {
    pub fn new(id: OpportunityId, edit: EditState) -> Self {
        let values = Field::ALL.iter().map(|f| f.get(&edit.effective)).collect();
        let a = edit.annotations.clone();
        Self {
            id,
            values,
            overridden: edit.overrides.iter().map(|o| o.field).collect(),
            reset: BTreeSet::new(),
            notes: a.notes.clone().unwrap_or_default(),
            priority: a.priority,
            custom: a.custom.clone(),
            annotations_at_open: a,
            detected: edit.detected,
            effective: edit.effective,
            row: 0,
            editing: None,
            error: None,
            confirm_discard: false,
        }
    }

    pub fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = Field::ALL.iter().copied().map(Row::Field).collect();
        rows.push(Row::Notes);
        rows.push(Row::Priority);
        rows.extend((0..self.custom.len()).map(Row::Custom));
        rows.push(Row::AddCustom);
        rows
    }

    pub fn current_row(&self) -> Row {
        let rows = self.rows();
        rows[self.row.min(rows.len() - 1)]
    }

    pub fn value(&self, field: Field) -> &str {
        &self.values[field_index(field)]
    }

    pub fn is_manual(&self, field: Field) -> bool {
        !self.reset.contains(&field) && (self.overridden.contains(&field) || self.is_changed(field))
    }

    pub fn is_changed(&self, field: Field) -> bool {
        self.value(field) != field.get(&self.effective)
    }

    fn annotations(&self) -> Annotations {
        Annotations {
            notes: Some(self.notes.trim().to_owned()).filter(|n| !n.is_empty()),
            priority: self.priority,
            custom: self.custom.clone(),
        }
    }

    pub fn is_dirty(&self) -> bool {
        !self.reset.is_empty()
            || Field::ALL.iter().any(|f| self.is_changed(*f))
            || self.annotations() != self.annotations_at_open
    }

    /// Validates every changed value against a copy of the detected data.
    fn validate(&self) -> Result<Vec<(Field, String)>, String> {
        let mut probe = self.detected.clone();
        let mut changed = Vec::new();
        for field in Field::ALL {
            if self.is_changed(*field) && !self.reset.contains(field) {
                let value = self.value(*field).to_owned();
                field.set(&mut probe, &value).map_err(|e| e.to_string())?;
                changed.push((*field, value));
            }
        }
        Ok(changed)
    }

    fn set_value(&mut self, field: Field, value: String) {
        self.reset.remove(&field);
        self.values[field_index(field)] = value;
    }

    fn cycle(&mut self, field: Field, forward: bool) {
        let FieldKind::Choice(options) = field.kind() else {
            return;
        };
        let current = options
            .iter()
            .position(|o| *o == self.value(field))
            .unwrap_or(0);
        let next = if forward {
            (current + 1) % options.len()
        } else {
            (current + options.len() - 1) % options.len()
        };
        self.set_value(field, options[next].to_owned());
    }

    fn cycle_priority(&mut self, forward: bool) {
        let order = [
            None,
            Some(Priority::Low),
            Some(Priority::Medium),
            Some(Priority::High),
        ];
        let i = order.iter().position(|p| *p == self.priority).unwrap_or(0);
        let next = if forward { (i + 1) % 4 } else { (i + 3) % 4 };
        self.priority = order[next];
    }

    fn start_editing(&mut self) {
        let text = match self.current_row() {
            Row::Field(field) => {
                if matches!(field.kind(), FieldKind::Choice(_)) {
                    self.cycle(field, true);
                    return;
                }
                inline(self.value(field))
            }
            Row::Notes => inline(&self.notes),
            Row::Priority => {
                self.cycle_priority(true);
                return;
            }
            Row::Custom(i) => format!("{}={}", self.custom[i].0, self.custom[i].1),
            Row::AddCustom => String::new(),
        };
        self.editing = Some(LineInput::with_value(&text));
    }

    fn commit_editing(&mut self, text: &str) -> Result<(), String> {
        match self.current_row() {
            Row::Field(field) => {
                let text = multiline(text);
                let mut probe = self.detected.clone();
                field.set(&mut probe, &text).map_err(|e| e.to_string())?;
                self.set_value(field, text.trim().to_owned());
            }
            Row::Notes => self.notes = multiline(text),
            Row::Priority => {}
            row @ (Row::Custom(_) | Row::AddCustom) => {
                let (key, value) = text.split_once('=').ok_or("Use the form name=value")?;
                let pair = (key.trim().to_owned(), value.trim().to_owned());
                if pair.0.is_empty() {
                    return Err("The custom field needs a name".into());
                }
                match row {
                    Row::Custom(i) => self.custom[i] = pair,
                    _ => self.custom.push(pair),
                }
            }
        }
        Ok(())
    }

    pub fn on_key(&mut self, key: KeyEvent) -> EditorOutcome {
        if self.confirm_discard {
            self.confirm_discard = false;
            return if matches!(key.code, KeyCode::Char('y' | 'Y')) {
                EditorOutcome::Close
            } else {
                EditorOutcome::Continue
            };
        }
        if let Some(mut input) = self.editing.take() {
            match key.code {
                KeyCode::Esc => self.error = None,
                KeyCode::Enter => match self.commit_editing(&input.value()) {
                    Ok(()) => self.error = None,
                    Err(e) => {
                        self.error = Some(e);
                        self.editing = Some(input);
                    }
                },
                _ => {
                    input.handle(key);
                    self.editing = Some(input);
                }
            }
            return EditorOutcome::Continue;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let last = self.rows().len() - 1;
        match key.code {
            KeyCode::Char('s') if ctrl => return self.save(),
            KeyCode::Esc | KeyCode::Char('q') => {
                if self.is_dirty() {
                    self.confirm_discard = true;
                } else {
                    return EditorOutcome::Close;
                }
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.row = (self.row + 1).min(last);
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                self.row = self.row.saturating_sub(1);
            }
            KeyCode::PageDown => self.row = (self.row + 10).min(last),
            KeyCode::PageUp => self.row = self.row.saturating_sub(10),
            KeyCode::Enter | KeyCode::Char('i') => self.start_editing(),
            KeyCode::Right | KeyCode::Left => {
                let forward = key.code == KeyCode::Right;
                match self.current_row() {
                    Row::Field(field) => self.cycle(field, forward),
                    Row::Priority => self.cycle_priority(forward),
                    _ => {}
                }
            }
            KeyCode::Char('r') => {
                if let Row::Field(field) = self.current_row() {
                    let detected = field.get(&self.detected);
                    self.values[field_index(field)] = detected;
                    if self.overridden.contains(&field) {
                        self.reset.insert(field);
                    }
                }
            }
            KeyCode::Char('+') => {
                self.row = last;
                self.editing = Some(LineInput::default());
            }
            KeyCode::Char('x') => {
                if let Row::Custom(i) = self.current_row() {
                    self.custom.remove(i);
                }
            }
            _ => {}
        }
        EditorOutcome::Continue
    }

    fn save(&mut self) -> EditorOutcome {
        match self.validate() {
            Ok(values) => EditorOutcome::Save {
                id: self.id,
                values,
                reset: self.reset.iter().copied().collect(),
                annotations: self.annotations(),
            },
            Err(e) => {
                self.error = Some(e);
                EditorOutcome::Continue
            }
        }
    }
}

/// Paragraph and line breaks shown in the one-line input: `¶` separates
/// paragraphs, `↵` marks a line break. [`multiline`] turns them back.
pub fn inline(text: &str) -> String {
    text.replace("\n\n", " ¶ ").replace('\n', " ↵ ")
}

/// Inverse of [`inline`]: what the user typed, with breaks restored.
pub fn multiline(text: &str) -> String {
    let lines: Vec<&str> = text.split('¶').map(str::trim).collect();
    lines
        .iter()
        .map(|p| p.split('↵').map(str::trim).collect::<Vec<_>>().join("\n"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use sentri_app::Override;
    use sentri_core::{CanonicalUrl, Category, SourceKind};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn typed(editor: &mut EditorState, text: &str) {
        for c in text.chars() {
            editor.on_key(key(KeyCode::Char(c)));
        }
    }

    fn editor() -> EditorState {
        let url = CanonicalUrl::parse("https://ig.example/p/1").unwrap();
        let detected = Opportunity::new(
            SourceKind::Manual,
            "manual",
            url.as_str(),
            &url,
            "Post",
            Utc::now(),
        );
        let mut effective = detected.clone();
        effective.title = "Mine".into();
        EditorState::new(
            OpportunityId(7),
            EditState {
                detected,
                effective,
                overrides: vec![Override {
                    field: Field::Title,
                    value: "Mine".into(),
                    edited_at: Utc::now(),
                }],
                annotations: Annotations::default(),
            },
        )
    }

    fn go_to(e: &mut EditorState, field: Field) {
        e.row = field_index(field);
    }

    #[test]
    fn edits_text_and_choices_then_saves_only_changes() {
        let mut e = editor();
        go_to(&mut e, Field::Location);
        e.on_key(key(KeyCode::Enter));
        typed(&mut e, "Cusco");
        e.on_key(key(KeyCode::Enter));
        go_to(&mut e, Field::Category);
        e.on_key(key(KeyCode::Right));
        assert_eq!(e.value(Field::Category), "event");
        assert!(e.is_dirty());

        let outcome = e.on_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        let EditorOutcome::Save { values, reset, .. } = outcome else {
            panic!("expected save");
        };
        assert_eq!(
            values,
            vec![
                (Field::Category, "event".to_owned()),
                (Field::Location, "Cusco".to_owned()),
            ]
        );
        assert_eq!(reset, Vec::<Field>::new());
    }

    #[test]
    fn rejects_invalid_values_inline() {
        let mut e = editor();
        go_to(&mut e, Field::Deadline);
        e.on_key(key(KeyCode::Enter));
        typed(&mut e, "next friday");
        e.on_key(key(KeyCode::Enter));
        assert!(e.error.is_some());
        assert!(e.editing.is_some(), "stays in edit mode to fix it");
        assert_eq!(e.value(Field::Deadline), "");
    }

    #[test]
    fn resets_manual_fields_to_detected() {
        let mut e = editor();
        go_to(&mut e, Field::Title);
        assert!(e.is_manual(Field::Title));
        e.on_key(key(KeyCode::Char('r')));
        assert_eq!(e.value(Field::Title), "Post");
        assert!(!e.is_manual(Field::Title));
        let EditorOutcome::Save { values, reset, .. } =
            e.on_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))
        else {
            panic!("expected save");
        };
        assert_eq!(reset, vec![Field::Title]);
        assert_eq!(values, Vec::new());
    }

    #[test]
    fn notes_priority_and_custom_fields() {
        let mut e = editor();
        e.on_key(key(KeyCode::Char('+')));
        typed(&mut e, "mentor=Rosa");
        e.on_key(key(KeyCode::Enter));
        assert_eq!(e.custom, vec![("mentor".to_owned(), "Rosa".to_owned())]);
        e.row = e.rows().iter().position(|r| *r == Row::Priority).unwrap();
        e.on_key(key(KeyCode::Right));
        assert_eq!(e.priority, Some(Priority::Low));
        e.row = e.rows().iter().position(|r| *r == Row::Custom(0)).unwrap();
        e.on_key(key(KeyCode::Char('x')));
        assert_eq!(e.custom, Vec::new());
    }

    #[test]
    fn asks_before_discarding_changes() {
        let mut e = editor();
        assert_eq!(e.on_key(key(KeyCode::Esc)), EditorOutcome::Close);
        let mut e = editor();
        go_to(&mut e, Field::Modality);
        e.on_key(key(KeyCode::Right));
        assert_eq!(e.on_key(key(KeyCode::Esc)), EditorOutcome::Continue);
        assert!(e.confirm_discard);
        assert_eq!(e.on_key(key(KeyCode::Char('n'))), EditorOutcome::Continue);
        e.on_key(key(KeyCode::Esc));
        assert_eq!(e.on_key(key(KeyCode::Char('y'))), EditorOutcome::Close);
    }

    #[test]
    fn category_values_are_valid_choices() {
        assert!(
            matches!(Field::Category.kind(), FieldKind::Choice(o) if o.contains(&Category::Hackathon.as_str()))
        );
    }

    #[test]
    fn line_breaks_survive_the_one_line_input() {
        let text = "Intro\n\n• one\n• two";
        assert_eq!(inline(text), "Intro ¶ • one ↵ • two");
        assert_eq!(multiline(&inline(text)), text);
        assert_eq!(multiline("plain text"), "plain text");
    }
}
