//! Minimal single-line text input (cursor, insert, delete, word delete).
//!
//! Small enough to own instead of pulling `tui-input`, which would couple us
//! to its crossterm version.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Maximum characters accepted, to keep pasted junk bounded.
const MAX_CHARS: usize = 2_048;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineInput {
    chars: Vec<char>,
    /// Cursor position in characters.
    cursor: usize,
}

impl LineInput {
    pub fn with_value(value: &str) -> Self {
        let chars: Vec<char> = value.chars().take(MAX_CHARS).collect();
        Self {
            cursor: chars.len(),
            chars,
        }
    }

    pub fn value(&self) -> String {
        self.chars.iter().collect()
    }

    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn clear(&mut self) {
        self.chars.clear();
        self.cursor = 0;
    }

    /// Applies an editing key. Returns `true` if the key was consumed.
    pub fn handle(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('u') if ctrl => {
                self.chars.drain(..self.cursor);
                self.cursor = 0;
            }
            KeyCode::Char('w') if ctrl => self.delete_word(),
            KeyCode::Char('a') if ctrl => self.cursor = 0,
            KeyCode::Char('e') if ctrl => self.cursor = self.chars.len(),
            KeyCode::Char(c) if !ctrl && !c.is_control() => {
                if self.chars.len() < MAX_CHARS {
                    self.chars.insert(self.cursor, c);
                    self.cursor += 1;
                }
            }
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.chars.remove(self.cursor);
            }
            KeyCode::Delete if self.cursor < self.chars.len() => {
                self.chars.remove(self.cursor);
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.chars.len()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.chars.len(),
            KeyCode::Backspace | KeyCode::Delete => {}
            _ => return false,
        }
        true
    }

    fn delete_word(&mut self) {
        let mut start = self.cursor;
        while start > 0 && self.chars[start - 1].is_whitespace() {
            start -= 1;
        }
        while start > 0 && !self.chars[start - 1].is_whitespace() {
            start -= 1;
        }
        self.chars.drain(start..self.cursor);
        self.cursor = start;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn typed(s: &str) -> LineInput {
        let mut input = LineInput::default();
        for c in s.chars() {
            input.handle(key(KeyCode::Char(c)));
        }
        input
    }

    #[test]
    fn edits_at_cursor() {
        let mut input = typed("rst");
        input.handle(key(KeyCode::Left));
        input.handle(key(KeyCode::Left));
        input.handle(key(KeyCode::Char('u')));
        assert_eq!(input.value(), "rust");
        input.handle(key(KeyCode::End));
        input.handle(key(KeyCode::Backspace));
        assert_eq!(input.value(), "rus");
        input.handle(key(KeyCode::Home));
        input.handle(key(KeyCode::Delete));
        assert_eq!(input.value(), "us");
    }

    #[test]
    fn supports_readline_shortcuts_and_unicode() {
        let mut input = typed("beca maestría ñ");
        input.handle(ctrl('w'));
        assert_eq!(input.value(), "beca maestría ");
        input.handle(ctrl('w'));
        assert_eq!(input.value(), "beca ");
        input.handle(ctrl('u'));
        assert!(input.is_empty());
    }

    #[test]
    fn ignores_unrelated_keys() {
        let mut input = typed("a");
        assert!(!input.handle(key(KeyCode::Enter)));
        assert!(!input.handle(key(KeyCode::Esc)));
        assert_eq!(input.value(), "a");
    }
}
