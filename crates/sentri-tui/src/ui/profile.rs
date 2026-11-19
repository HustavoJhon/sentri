//! GitHub identity at the bottom of the sidebar.
//!
//! Tall terminals get a card with a square avatar on top and the name,
//! handle and stats centered below; shorter ones a single row with a small
//! avatar. A cell is about twice as tall as it is wide, so a square avatar
//! is `2·h` columns by `h` rows, both with half blocks (two pixels per cell)
//! and with graphics protocols.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use sentri_app::{GitHubProfile, ImageMode, Thumbnail};

use super::fit;
use super::widgets::panel;
use crate::graphics::Fit;
use crate::state::{ProfileState, State};
use crate::theme::Theme;

/// Avatar of 5 rows plus three lines of text, inside the border.
pub const FULL_HEIGHT: u16 = 11;
/// One row of avatar (3 rows) next to the text.
pub const COMPACT_HEIGHT: u16 = 5;

pub fn render(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let block = panel(theme, "GitHub", false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let muted = |t: &str| Line::from(Span::styled(t.to_owned(), theme.muted()));
    let (profile, avatar) = match &state.profile {
        ProfileState::Ready(profile, avatar) => (profile, avatar.as_ref()),
        ProfileState::Loading => {
            frame.render_widget(Paragraph::new(muted("loading…")), inner);
            return;
        }
        ProfileState::NotConfigured => {
            frame.render_widget(
                Paragraph::new(vec![
                    muted("No GitHub user."),
                    muted("Settings › Profile"),
                    muted("or [github] username"),
                ]),
                inner,
            );
            return;
        }
        ProfileState::Unavailable(_) => {
            frame.render_widget(
                Paragraph::new(vec![muted("GitHub profile"), muted("unavailable")]),
                inner,
            );
            return;
        }
    };
    if inner.height >= FULL_HEIGHT - 2 {
        full(frame, inner, state, theme, profile, avatar);
    } else {
        compact(frame, inner, state, theme, profile, avatar);
    }
}

fn identity<'a>(theme: &Theme, profile: &GitHubProfile, width: usize) -> Vec<Line<'a>> {
    let name = profile.name.as_deref().unwrap_or(&profile.login);
    vec![
        Line::from(Span::styled(
            fit(name, width),
            Style::new().fg(theme.p.fg).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            fit(&format!("@{}", profile.login), width),
            Style::new().fg(theme.p.aqua),
        )),
        Line::from(Span::styled(
            fit(&stats(profile, width), width),
            theme.muted(),
        )),
    ]
}

/// Repositories and followers, or just repositories when both do not fit.
fn stats(profile: &GitHubProfile, width: usize) -> String {
    let full = format!(
        "{} repos · {} followers",
        profile.public_repos, profile.followers
    );
    if full.chars().count() <= width {
        full
    } else {
        format!("{} repos", profile.public_repos)
    }
}

/// Picture, or the initials on a quiet tile when there is none to show.
fn avatar(
    frame: &mut Frame,
    area: Rect,
    state: &State,
    theme: &Theme,
    profile: &GitHubProfile,
    image: Option<&Thumbnail>,
) {
    if let Some(image) = image.filter(|_| state.ui.images != ImageMode::Off) {
        let used = state.graphics.draw(
            frame.buffer_mut(),
            area,
            &profile.avatar_cache_key(),
            image,
            Fit::Cover,
            state.ui.images,
            state.ui.indexed_colors,
        );
        if used.width > 0 {
            return;
        }
    }
    frame.render_widget(Block::new().style(Style::new().bg(theme.p.bg1)), area);
    let middle = Rect {
        y: area.y + area.height / 2,
        height: 1,
        ..area
    };
    frame.render_widget(
        Paragraph::new(Span::styled(
            profile.initials(),
            Style::new()
                .fg(theme.p.orange)
                .bg(theme.p.bg1)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        middle,
    );
}

fn full(
    frame: &mut Frame,
    inner: Rect,
    state: &State,
    theme: &Theme,
    profile: &GitHubProfile,
    image: Option<&Thumbnail>,
) {
    let rows = inner.height.saturating_sub(4).min(5);
    let cols = (rows * 2).min(inner.width);
    let pic = Rect::new(inner.x + (inner.width - cols) / 2, inner.y, cols, rows);
    avatar(frame, pic, state, theme, profile, image);
    let text = Rect {
        y: inner.y + rows + 1,
        height: inner.height.saturating_sub(rows + 1),
        ..inner
    };
    frame.render_widget(
        Paragraph::new(identity(theme, profile, usize::from(text.width)))
            .alignment(Alignment::Center),
        text,
    );
}

fn compact(
    frame: &mut Frame,
    inner: Rect,
    state: &State,
    theme: &Theme,
    profile: &GitHubProfile,
    image: Option<&Thumbnail>,
) {
    let rows = inner.height.min(3);
    let pic = Rect::new(inner.x, inner.y, rows * 2, rows);
    avatar(frame, pic, state, theme, profile, image);
    let text = Rect {
        x: inner.x + pic.width + 1,
        width: inner.width.saturating_sub(pic.width + 1),
        ..inner
    };
    frame.render_widget(
        Paragraph::new(identity(theme, profile, usize::from(text.width))),
        text,
    );
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn profile() -> GitHubProfile {
        GitHubProfile {
            login: "octocat".into(),
            name: Some("The Octocat With A Very Long Display Name".into()),
            html_url: "https://github.com/octocat".into(),
            avatar_url: "https://avatars.githubusercontent.com/u/1".into(),
            public_repos: 8,
            followers: 10,
            updated_at: None,
        }
    }

    fn screen(height: u16, avatar: Option<Thumbnail>, images: ImageMode) -> Vec<String> {
        let mut state = State::default();
        state.ui.images = images;
        state.profile = ProfileState::Ready(Box::new(profile()), avatar);
        let mut terminal = Terminal::new(TestBackend::new(24, height)).unwrap();
        terminal
            .draw(|f| render(f, f.area(), &state, &Theme::default()))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        buffer
            .content()
            .chunks(24)
            .map(|row| row.iter().map(ratatui::buffer::Cell::symbol).collect())
            .collect()
    }

    #[test]
    fn full_card_has_a_square_avatar_and_centered_identity() {
        let avatar = Thumbnail::from_pixels(2, 2, &[[200, 100, 50]; 4]);
        let rows = screen(FULL_HEIGHT, avatar, ImageMode::Halfblock);
        assert!(rows[0].contains("GitHub"));
        let avatar_row = &rows[1];
        assert_eq!(avatar_row.matches('▀').count(), 10, "10×5 cells: square");
        assert!(
            rows.iter().any(|r| r.contains("The Octocat With A …")),
            "{rows:#?}"
        );
        assert!(rows.iter().any(|r| r.contains("@octocat")));
        assert!(rows.iter().any(|r| r.contains("8 repos")));
        assert!(rows.iter().all(|r| !r.contains("follo…")), "no cut words");
    }

    #[test]
    fn falls_back_to_initials_without_an_image() {
        for (avatar, mode) in [
            (None, ImageMode::Halfblock),
            (Thumbnail::from_pixels(1, 1, &[[0; 3]]), ImageMode::Off),
        ] {
            let rows = screen(FULL_HEIGHT, avatar, mode);
            assert!(rows.iter().any(|r| r.contains("TO")), "{rows:#?}");
            assert!(!rows.iter().any(|r| r.contains('▀')));
        }
    }

    #[test]
    fn compact_card_fits_short_sidebars() {
        let rows = screen(COMPACT_HEIGHT, None, ImageMode::Halfblock);
        assert!(
            rows[1].contains("TO") || rows[2].contains("TO"),
            "{rows:#?}"
        );
        assert!(rows[1].contains("The Octocat"));
        assert!(rows[2].contains("@octocat"));
    }
}
