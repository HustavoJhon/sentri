//! Draws the onboarding wizard.

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap};
use sentri_app::Lookup;
use sentri_core::Topic;

use crate::input::LineInput;
use crate::observer::{self, Eye, Part};
use crate::state::onboarding::{Field, GitHubCheck, Onboarding, Page, SCOPES};
use crate::theme::Theme;

/// A text field with a visible cursor when focused.
fn input_line<'a>(
    input: &LineInput,
    focused: bool,
    placeholder: &'a str,
    theme: &Theme,
) -> Vec<Span<'a>> {
    let value = input.value();
    if value.is_empty() && !focused {
        return vec![Span::styled(placeholder, theme.muted())];
    }
    let text = Style::new().fg(theme.p.fg);
    if !focused {
        return vec![Span::styled(value, text)];
    }
    let chars: Vec<char> = value.chars().collect();
    let cursor = input.cursor().min(chars.len());
    let before: String = chars[..cursor].iter().collect();
    let at: String = chars
        .get(cursor)
        .map_or_else(|| " ".to_owned(), char::to_string);
    let after: String = chars
        .get(cursor + 1..)
        .map(|c| c.iter().collect())
        .unwrap_or_default();
    vec![
        Span::styled(before, text),
        Span::styled(at, Style::new().fg(theme.p.bg0).bg(theme.p.yellow)),
        Span::styled(after, text),
    ]
}

fn check_line(check: &GitHubCheck, theme: &Theme) -> Option<Line<'static>> {
    let (text, color) = match check {
        GitHubCheck::Unchecked => return None,
        GitHubCheck::Checking(name) => (format!("Checking @{name} on GitHub…"), theme.p.gray),
        GitHubCheck::Checked(_, Lookup::Found(p)) => (
            format!(
                "Found @{}{} · {} public repos",
                p.login,
                p.name
                    .as_deref()
                    .map(|n| format!(" ({n})"))
                    .unwrap_or_default(),
                p.public_repos
            ),
            theme.p.green,
        ),
        GitHubCheck::Checked(name, Lookup::NotFound) => (
            format!("GitHub has no user @{name}; it is saved anyway, fix it in Settings"),
            theme.p.orange,
        ),
        GitHubCheck::Checked(name, Lookup::Invalid) => {
            (format!("@{name} is not a valid username"), theme.p.red)
        }
        GitHubCheck::Checked(_, Lookup::Unavailable(_)) => (
            "GitHub could not be reached; the name is kept and checked later".into(),
            theme.p.orange,
        ),
    };
    Some(Line::styled(text, Style::new().fg(color)))
}

fn checkbox<'a>(
    label: &'a str,
    on: bool,
    selected: bool,
    unicode: bool,
    theme: &Theme,
) -> Line<'a> {
    let mark = match (on, unicode) {
        (true, true) => "◉",
        (false, true) => "○",
        (true, false) => "[x]",
        (false, false) => "[ ]",
    };
    let pointer = if selected { "› " } else { "  " };
    let label_style = if selected {
        Style::new().fg(theme.p.yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme.p.fg)
    };
    Line::from(vec![
        Span::styled(pointer, theme.accent()),
        Span::styled(
            format!("{mark} "),
            Style::new().fg(if on { theme.p.green } else { theme.p.gray }),
        ),
        Span::styled(label, label_style),
    ])
}

fn welcome(unicode: bool, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = observer::frame(Eye::Open, 0, unicode)
        .into_iter()
        .skip(1)
        .take(6)
        .map(|segments| {
            Line::from(
                segments
                    .into_iter()
                    .map(|(part, text)| {
                        let color = match part {
                            Part::Wing => theme.p.aqua,
                            Part::Eye | Part::Ray => theme.p.orange,
                            _ => theme.p.yellow,
                        };
                        Span::styled(text, Style::new().fg(color))
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    lines.push(Line::default());
    let bullet = if unicode { "  · " } else { "  - " };
    lines.extend([
        Line::styled(
            "The Observer is awake. A few questions help it watch the right places:",
            Style::new().fg(theme.p.fg),
        ),
        Line::from(format!(
            "{bullet}your GitHub profile (optional, public data only)"
        )),
        Line::from(format!(
            "{bullet}where you are, to favor nearby opportunities"
        )),
        Line::from(format!("{bullet}how far to look, and what interests you")),
        Line::default(),
        Line::styled(
            "Answers stay in config.toml on this computer; every one can be skipped and \
             changed later in Settings.",
            theme.muted(),
        ),
    ]);
    lines
}

fn location(w: &Onboarding, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::styled(
            "Nearby opportunities rank higher. Every field is optional.",
            theme.muted(),
        ),
        Line::default(),
    ];
    for (i, field) in Field::ALL.iter().enumerate() {
        let focused = w.field == i;
        let label = Style::new().fg(if focused { theme.p.yellow } else { theme.p.fg4 });
        let placeholder = match field {
            Field::Country => "e.g. PE, MX, ES",
            Field::City => "e.g. Cusco",
            Field::Region => "optional",
            Field::Timezone => "e.g. America/Lima",
        };
        let mut spans = vec![
            Span::styled(if focused { "› " } else { "  " }, theme.accent()),
            Span::styled(format!("{:<16}", field.label()), label),
        ];
        spans.extend(
            input_line(&w.fields[i], focused, placeholder, theme)
                .into_iter()
                .map(|s| Span::styled(s.content.into_owned(), s.style)),
        );
        match field {
            Field::Country => match w.country_name() {
                Ok(Some(name)) => spans.push(Span::styled(format!("  → {name}"), theme.muted())),
                Ok(None) => {}
                Err(_) => spans.push(Span::styled(
                    "  unknown code",
                    Style::new().fg(theme.p.orange),
                )),
            },
            Field::Timezone if focused => {
                spans.push(Span::styled("  ←/→ suggestions", theme.muted()));
            }
            _ => {}
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn review(w: &Onboarding, theme: &Theme) -> Vec<Line<'static>> {
    let row = |label: &str, value: String| {
        let shown = if value.is_empty() {
            "—".to_owned()
        } else {
            value
        };
        Line::from(vec![
            Span::styled(format!("  {label:<16}"), theme.dim()),
            Span::styled(shown, Style::new().fg(theme.p.fg)),
        ])
    };
    let github = w.github.value().trim().to_owned();
    let country = match w.country_name() {
        Ok(Some(name)) => format!("{} ({name})", w.value(Field::Country).to_ascii_uppercase()),
        _ => w.value(Field::Country),
    };
    let scopes: Vec<&str> = SCOPES
        .iter()
        .zip(w.scopes)
        .filter(|(_, on)| *on)
        .map(|(label, _)| label.trim_end_matches(" opportunities"))
        .collect();
    let topics: Vec<&str> = w.chosen_topics().iter().map(|t| t.label()).collect();
    let mut lines = vec![
        Line::styled("Enter saves and opens your dashboard.", theme.muted()),
        Line::default(),
        row(
            "GitHub",
            if github.is_empty() {
                String::new()
            } else {
                format!("@{github}")
            },
        ),
    ];
    if let Some(check) = check_line(&w.check, theme) {
        lines.push(Line::from(
            [vec![Span::raw("                  ")], check.spans].concat(),
        ));
    }
    lines.extend([
        row("Country", country),
        row("City", w.value(Field::City)),
        row("Region", w.value(Field::Region)),
        row("Time zone", w.value(Field::Timezone)),
        row("Look for", scopes.join(", ")),
        row("Interests", topics.join(", ")),
    ]);
    lines
}

fn body(w: &Onboarding, unicode: bool, theme: &Theme) -> Vec<Line<'static>> {
    match w.page {
        Page::Welcome => welcome(unicode, theme),
        Page::GitHub => {
            let mut spans = vec![Span::styled("  @ ", theme.accent())];
            spans.extend(
                input_line(&w.github, true, "", theme)
                    .into_iter()
                    .map(|s| Span::styled(s.content.into_owned(), s.style)),
            );
            let mut lines = vec![
                Line::styled(
                    "Optional. SENTRI reads only your public profile and the languages of your \
                     public repositories, to show your avatar and favor open-source work in \
                     your stack. No token, no login.",
                    theme.muted(),
                ),
                Line::default(),
                Line::from(spans),
                Line::default(),
            ];
            lines.extend(check_line(&w.check, theme));
            lines
        }
        Page::Location => location(w, theme),
        Page::Discovery => {
            let mut lines = vec![
                Line::styled(
                    "Chosen ones rank higher. Nothing is hidden: international and remote \
                     opportunities stay visible even if you pick only local ones.",
                    theme.muted(),
                ),
                Line::default(),
            ];
            for (i, label) in SCOPES.iter().enumerate() {
                if i == 3 {
                    lines.push(Line::default());
                }
                lines.push(checkbox(
                    label,
                    w.scopes[i],
                    w.scope_cursor == i,
                    unicode,
                    theme,
                ));
            }
            lines
        }
        Page::Interests => {
            let mut lines = vec![
                Line::styled(
                    "Pick any number; matching items rank higher and show why.",
                    theme.muted(),
                ),
                Line::default(),
            ];
            for (i, topic) in Topic::ALL.iter().enumerate() {
                lines.push(checkbox(
                    topic.label(),
                    w.topics[i],
                    w.topic_cursor == i,
                    unicode,
                    theme,
                ));
            }
            lines
        }
        Page::Review => review(w, theme),
    }
}

fn hints(page: Page) -> &'static str {
    match page {
        Page::Welcome => "Enter begin · Esc skip for now",
        Page::GitHub => "type · Enter continue (empty skips) · Esc back",
        Page::Location => "↑↓/Tab field · Enter continue · Esc back",
        Page::Discovery | Page::Interests => {
            "↑↓ move · Space toggle · a all · Enter continue · Esc back"
        }
        Page::Review => "Enter save · Esc back",
    }
}

/// Draws the wizard over the whole screen.
pub fn render(frame: &mut Frame, w: &Onboarding, unicode: bool, theme: &Theme) {
    let area = frame.area();
    frame.render_widget(Block::new().style(theme.base()), area);
    let width = area.width.min(86);
    let height = area.height.min(26);
    let [column] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);
    let [panel] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(column);
    let step = w.step();
    let total = Page::ALL.len();
    let dots: String = (0..total)
        .map(|i| match (i <= step, unicode) {
            (true, true) => "●",
            (false, true) => "○",
            (true, false) => "#",
            (false, false) => ".",
        })
        .collect::<Vec<_>>()
        .join(" ");
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(if unicode {
            BorderType::Rounded
        } else {
            BorderType::Plain
        })
        .border_style(theme.border(true))
        .title(Line::from(vec![
            Span::styled(" SENTRI ", theme.badge(theme.p.orange)),
            Span::styled(format!(" Setup · {} ", w.page.title()), theme.title()),
        ]))
        .title_bottom(Line::styled(format!(" {} ", hints(w.page)), theme.muted()))
        .padding(Padding::new(2, 2, 1, 0))
        .style(theme.panel());
    let inner = block.inner(panel);
    frame.render_widget(block, panel);
    let [progress, content, error] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .areas(inner);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(dots, Style::new().fg(theme.p.yellow)),
            Span::styled(format!("   step {} of {total}", step + 1), theme.muted()),
        ])),
        progress,
    );
    frame.render_widget(
        Paragraph::new(body(w, unicode, theme)).wrap(Wrap { trim: false }),
        content,
    );
    let status = match (&w.error, w.finishing) {
        (Some(e), _) => Line::styled(e.clone(), Style::new().fg(theme.p.red)),
        (None, true) => Line::styled("Saving your profile…", theme.muted()),
        (None, false) => Line::styled(
            "Ctrl-C quits; answers saved so far are kept and onboarding resumes next time.",
            theme.muted(),
        ),
    };
    frame.render_widget(Paragraph::new(status).wrap(Wrap { trim: true }), error);
    if !unicode {
        super::ascii::asciify(frame.buffer_mut());
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    use sentri_app::Config;

    use super::*;
    use crate::theme::Palette;

    fn screen(w: &Onboarding, unicode: bool) -> String {
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal
            .draw(|f| render(f, w, unicode, &Theme::new(Palette::TRUECOLOR)))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .chunks(100)
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn wizard() -> Onboarding {
        let config = Config::parse(sentri_app::config::DEFAULT_CONFIG, Path::new("d")).unwrap();
        Onboarding::from_config(&config, Some("America/Lima".into()))
    }

    #[test]
    fn every_page_draws_its_content_and_progress() {
        let mut w = wizard();
        let expectations = [
            (Page::Welcome, "The Observer is awake"),
            (Page::GitHub, "reads only your public profile"),
            (Page::Location, "Country code"),
            (Page::Discovery, "International opportunities"),
            (Page::Interests, "Jobs and internships"),
            (Page::Review, "Enter saves and opens your dashboard"),
        ];
        for (i, (page, text)) in expectations.iter().enumerate() {
            w.page = *page;
            let s = screen(&w, true);
            assert!(s.contains(text), "{page:?}: {s}");
            assert!(s.contains(&format!("step {} of 6", i + 1)), "{page:?}");
            assert!(s.contains("Esc"), "{page:?} shows how to go back");
        }
    }

    #[test]
    fn location_shows_the_country_name_and_review_the_answers() {
        let mut w = wizard();
        w.page = Page::Location;
        let s = screen(&w, true);
        assert!(s.contains("PE"));
        assert!(s.contains("→ Peru"));
        assert!(s.contains("America/Lima"));
        w.page = Page::Interests;
        w.on_key(KeyEvent::from(KeyCode::Char(' ')));
        w.page = Page::Review;
        let s = screen(&w, false);
        assert!(s.contains("PE (Peru)"));
        assert!(s.contains("Events"));
        assert!(s.contains("Cusco"));
        assert!(s.is_ascii(), "ASCII mode draws no Unicode");
    }
}
