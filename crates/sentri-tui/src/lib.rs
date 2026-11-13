//! SENTRI terminal interface.
//!
//! Built with Ratatui on Crossterm. Every glyph is plain Unicode, so the UI
//! works over SSH and in Android terminals without Nerd Fonts or graphics
//! protocols.

pub mod theme;

pub use theme::{Palette, Theme};
