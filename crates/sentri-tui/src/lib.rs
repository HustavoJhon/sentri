//! SENTRI terminal interface.
//!
//! Built with Ratatui on Crossterm. Every glyph is plain Unicode, so the UI
//! works over SSH and in Android terminals without Nerd Fonts or graphics
//! protocols.

pub mod input;
mod runtime;
pub mod state;
pub mod theme;
pub mod ui;

pub use runtime::{TuiError, run};
pub use theme::{Palette, Theme};
