//! SENTRI terminal interface.
//!
//! Built with Ratatui on Crossterm. Every glyph is plain Unicode, so the UI
//! works over SSH and in Android terminals without Nerd Fonts. Images use
//! the Kitty, Sixel or iTerm2 graphics protocol when the terminal supports
//! one, and colored half blocks otherwise.

pub mod graphics;
pub mod input;
mod runtime;
pub mod state;
pub mod theme;
pub mod ui;

pub use runtime::{TuiError, run};
pub use theme::{Palette, Theme};
