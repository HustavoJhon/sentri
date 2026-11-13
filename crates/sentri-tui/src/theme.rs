//! Gruvbox Dark palette and semantic styles.
//!
//! Truecolor is used when the terminal advertises it (`COLORTERM`); otherwise
//! the closest xterm-256 indexes are used, which also work over SSH and in
//! Android terminals.

use ratatui::style::{Color, Modifier, Style};
use sentri_core::{Category, SourceKind};

/// Gruvbox Dark colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub bg0_h: Color,
    pub bg0: Color,
    pub bg1: Color,
    pub bg2: Color,
    pub bg3: Color,
    pub fg: Color,
    pub fg4: Color,
    pub gray: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub purple: Color,
    pub aqua: Color,
    pub orange: Color,
}

impl Palette {
    pub const TRUECOLOR: Self = Self {
        bg0_h: Color::Rgb(0x1d, 0x20, 0x21),
        bg0: Color::Rgb(0x28, 0x28, 0x28),
        bg1: Color::Rgb(0x3c, 0x38, 0x36),
        bg2: Color::Rgb(0x50, 0x49, 0x45),
        bg3: Color::Rgb(0x66, 0x5c, 0x54),
        fg: Color::Rgb(0xeb, 0xdb, 0xb2),
        fg4: Color::Rgb(0xa8, 0x99, 0x84),
        gray: Color::Rgb(0x92, 0x83, 0x74),
        red: Color::Rgb(0xfb, 0x49, 0x34),
        green: Color::Rgb(0xb8, 0xbb, 0x26),
        yellow: Color::Rgb(0xfa, 0xbd, 0x2f),
        blue: Color::Rgb(0x83, 0xa5, 0x98),
        purple: Color::Rgb(0xd3, 0x86, 0x9b),
        aqua: Color::Rgb(0x8e, 0xc0, 0x7c),
        orange: Color::Rgb(0xfe, 0x80, 0x19),
    };

    /// Nearest xterm-256 colors (as used by the official gruvbox vim theme).
    pub const INDEXED: Self = Self {
        bg0_h: Color::Indexed(234),
        bg0: Color::Indexed(235),
        bg1: Color::Indexed(237),
        bg2: Color::Indexed(239),
        bg3: Color::Indexed(241),
        fg: Color::Indexed(223),
        fg4: Color::Indexed(246),
        gray: Color::Indexed(245),
        red: Color::Indexed(167),
        green: Color::Indexed(142),
        yellow: Color::Indexed(214),
        blue: Color::Indexed(109),
        purple: Color::Indexed(175),
        aqua: Color::Indexed(108),
        orange: Color::Indexed(208),
    };

    /// Picks truecolor when `COLORTERM` says it is supported.
    pub fn detect() -> Self {
        let truecolor =
            std::env::var("COLORTERM").is_ok_and(|v| matches!(v.as_str(), "truecolor" | "24bit"));
        if truecolor {
            Self::TRUECOLOR
        } else {
            Self::INDEXED
        }
    }
}

/// Semantic styles used by every widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub p: Palette,
}

impl Theme {
    pub const fn new(p: Palette) -> Self {
        Self { p }
    }

    pub fn base(&self) -> Style {
        Style::new().fg(self.p.fg).bg(self.p.bg0)
    }

    pub fn panel(&self) -> Style {
        Style::new().fg(self.p.fg).bg(self.p.bg0)
    }

    pub fn border(&self, focused: bool) -> Style {
        Style::new().fg(if focused { self.p.yellow } else { self.p.bg3 })
    }

    pub fn title(&self) -> Style {
        Style::new().fg(self.p.yellow).add_modifier(Modifier::BOLD)
    }

    pub fn muted(&self) -> Style {
        Style::new().fg(self.p.gray)
    }

    pub fn dim(&self) -> Style {
        Style::new().fg(self.p.fg4)
    }

    pub fn selected(&self) -> Style {
        Style::new().bg(self.p.bg1).add_modifier(Modifier::BOLD)
    }

    pub fn accent(&self) -> Style {
        Style::new().fg(self.p.orange).add_modifier(Modifier::BOLD)
    }

    pub fn good(&self) -> Style {
        Style::new().fg(self.p.green)
    }

    pub fn warn(&self) -> Style {
        Style::new().fg(self.p.yellow)
    }

    pub fn bad(&self) -> Style {
        Style::new().fg(self.p.red)
    }

    pub fn key(&self) -> Style {
        Style::new()
            .fg(self.p.bg0)
            .bg(self.p.fg4)
            .add_modifier(Modifier::BOLD)
    }

    /// Reverse-video badge in the given color.
    pub fn badge(&self, color: Color) -> Style {
        Style::new()
            .fg(self.p.bg0)
            .bg(color)
            .add_modifier(Modifier::BOLD)
    }

    /// Brand-like color for each platform badge.
    pub const fn source_color(&self, kind: SourceKind) -> Color {
        match kind {
            SourceKind::Reddit
            | SourceKind::Devpost
            | SourceKind::HackerNews
            | SourceKind::Eventbrite => self.p.orange,
            SourceKind::LinkedIn | SourceKind::MicrosoftLearn | SourceKind::Meetup => self.p.blue,
            SourceKind::GitHub | SourceKind::X | SourceKind::Web => self.p.fg4,
            SourceKind::Luma | SourceKind::Kaggle => self.p.aqua,
            SourceKind::Gdg => self.p.green,
            SourceKind::Rss | SourceKind::ICal => self.p.yellow,
            SourceKind::Manual => self.p.purple,
        }
    }

    pub const fn category_color(&self, category: Category) -> Color {
        match category {
            Category::Hackathon | Category::Competition => self.p.red,
            Category::Job | Category::Internship => self.p.blue,
            Category::Course | Category::Training => self.p.green,
            Category::Scholarship | Category::Fellowship | Category::Exchange => self.p.purple,
            Category::Event | Category::Conference | Category::Workshop | Category::Meetup => {
                self.p.aqua
            }
            Category::Volunteering | Category::Call => self.p.yellow,
            Category::OpenSource => self.p.orange,
            Category::Other => self.p.gray,
        }
    }

    /// Color for a relevance score.
    pub const fn score_color(&self, score: i32) -> Color {
        match score {
            i32::MIN..=-1 => self.p.red,
            0..=19 => self.p.gray,
            20..=49 => self.p.yellow,
            _ => self.p.green,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(Palette::TRUECOLOR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_source_and_category_has_a_color() {
        let t = Theme::default();
        for kind in SourceKind::ALL {
            assert_ne!(t.source_color(kind), Color::Reset);
        }
        for category in Category::ALL {
            assert_ne!(t.category_color(category), Color::Reset);
        }
        assert_eq!(t.score_color(80), t.p.green);
        assert_eq!(t.score_color(-5), t.p.red);
    }
}
