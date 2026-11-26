//! Draws the onboarding wizard in the same visual language as the startup
//! console: dark panel, orange accent bar, a stepper across the top and the
//! rotating Observer on the Welcome page.

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap};
use sentri_app::Lookup;
use sentri_core::Topic;

use crate::input::LineInput;
use crate::observer::{self, Eye};
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

/// The Welcome text: what SENTRI is and what the next pages ask.
fn welcome(unicode: bool, theme: &Theme) -> Vec<Line<'static>> {
    let bullet = if unicode { "  › " } else { "  > " };
    let item = |text: &'static str| {
        Line::from(vec![
            Span::styled(bullet, Style::new().fg(theme.p.orange)),
            Span::styled(text, Style::new().fg(theme.p.fg)),
        ])
    };
    vec![
        Line::styled(
            "Welcome. The Observer is awake.",
            Style::new().fg(theme.p.yellow).add_modifier(Modifier::BOLD),
        ),
        Line::default(),
        Line::styled(
            "SENTRI watches jobs, events, hackathons, scholarships and open \
             source, and ranks what fits you.",
            Style::new().fg(theme.p.fg4),
        ),
        Line::default(),
        Line::styled("Five short pages tell it where to look:", theme.muted()),
        item("your GitHub profile (optional, public only)"),
        item("where you are, to favor nearby events"),
        item("how far to look and what interests you"),
        Line::default(),
        Line::styled(
            "Answers stay in config.toml on this computer; \
             skip any, change them later in Settings.",
            theme.dim(),
        ),
        Line::default(),
        Line::from(vec![
            Span::styled(" Enter ", theme.badge(theme.p.yellow)),
            Span::styled(
                if unicode {
                    "  begin setup  →"
                } else {
                    "  begin setup  ->"
                },
                Style::new().fg(theme.p.yellow),
            ),
        ]),
    ]
}

/// Welcome page with the rotating Observer beside the wordmark and text;
/// text only when the panel is narrow.
fn welcome_page(
    frame: &mut Frame,
    area: Rect,
    w: &Onboarding,
    unicode: bool,
    motion: bool,
    theme: &Theme,
) {
    if area.width < 80 || area.height < 16 {
        frame.render_widget(
            Paragraph::new(welcome(unicode, theme)).wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let [art, _, text] = Layout::horizontal([
        Constraint::Length(40),
        Constraint::Length(2),
        Constraint::Min(0),
    ])
    .areas(area);
    let rows = art.height.min(16);
    let target = super::startup::centered(art, 40, rows);
    // A still frame without motion: the tick only drives animation.
    let tick = if motion { w.tick } else { 0 };
    let mut scene = observer::scene(Eye::Open, tick, 1.0, motion, unicode);
    scene.radar = false;
    let canvas = observer::render::render(
        &scene,
        usize::from(target.width),
        usize::from(target.height),
    );
    super::startup::blit(frame.buffer_mut(), target, &canvas, theme);
    let mark_rows = if unicode { 3 } else { 1 };
    let [mark, tag, _, body] = Layout::vertical([
        Constraint::Length(mark_rows),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(text);
    frame.render_widget(
        Paragraph::new(super::startup::wordmark(unicode, theme)),
        mark,
    );
    frame.render_widget(
        Paragraph::new(Line::styled(
            super::startup::TAGLINE,
            Style::new().fg(theme.p.fg4),
        )),
        tag,
    );
    frame.render_widget(
        Paragraph::new(welcome(unicode, theme)).wrap(Wrap { trim: false }),
        body,
    );
}

/// Page names across the top: done in green, the current one highlighted,
/// the rest dimmed. Falls back to dots when narrow.
fn stepper(w: &Onboarding, width: u16, unicode: bool, theme: &Theme) -> Line<'static> {
    let step = w.step();
    let total = Page::ALL.len();
    let count = Span::styled(format!("   step {} of {total}", step + 1), theme.muted());
    let names: Vec<&str> = Page::ALL
        .iter()
        .map(|p| match p {
            Page::GitHub => "GitHub",
            other => other.title(),
        })
        .collect();
    let divider = if unicode { " ─ " } else { " - " };
    let full: usize = names.iter().map(|n| n.len()).sum::<usize>() + 3 * (total - 1) + 16;
    let mut spans = Vec::new();
    if usize::from(width) >= full {
        for (i, name) in names.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(divider, Style::new().fg(theme.p.bg3)));
            }
            let style = match i.cmp(&step) {
                std::cmp::Ordering::Less => Style::new().fg(theme.p.green),
                std::cmp::Ordering::Equal => {
                    Style::new().fg(theme.p.yellow).add_modifier(Modifier::BOLD)
                }
                std::cmp::Ordering::Greater => theme.dim(),
            };
            spans.push(Span::styled((*name).to_owned(), style));
        }
    } else {
        let dots: String = (0..total)
            .map(|i| match (i <= step, unicode) {
                (true, true) => "●",
                (false, true) => "○",
                (true, false) => "#",
                (false, false) => ".",
            })
            .collect::<Vec<_>>()
            .join(" ");
        spans.push(Span::styled(dots, Style::new().fg(theme.p.yellow)));
    }
    spans.push(count);
    Line::from(spans)
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
        // Drawn by `welcome_page`; the text alone serves narrow panels.
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

/// Draws the wizard over the whole screen. `motion` turns the Observer on
/// the Welcome page (off in static mode).
pub fn render(frame: &mut Frame, w: &Onboarding, unicode: bool, motion: bool, theme: &Theme) {
    let area = frame.area();
    frame.render_widget(Block::new().style(theme.base()), area);
    let width = area.width.min(96);
    let height = area.height.min(28);
    let [column] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);
    let [panel] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(column);
    let glyph = if unicode { "◉" } else { "@" };
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(if unicode {
            BorderType::Rounded
        } else {
            BorderType::Plain
        })
        .border_style(Style::new().fg(theme.p.bg3))
        .title(Line::from(vec![
            Span::styled(format!(" {glyph} "), Style::new().fg(theme.p.orange)),
            Span::styled("SENTRI", theme.title()),
            Span::styled(format!(" · SETUP · {} ", w.page.title()), theme.muted()),
        ]))
        .title_bottom(Line::styled(format!(" {} ", hints(w.page)), theme.muted()))
        .padding(Padding::new(2, 2, 1, 0))
        .style(Style::new().bg(theme.p.bg0_h).fg(theme.p.fg));
    let inner = block.inner(panel);
    frame.render_widget(block, panel);
    let [progress, content, error] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .areas(inner);
    frame.render_widget(
        Paragraph::new(stepper(w, progress.width, unicode, theme)),
        progress,
    );
    if w.page == Page::Welcome {
        welcome_page(frame, content, w, unicode, motion, theme);
    } else {
        frame.render_widget(
            Paragraph::new(body(w, unicode, theme)).wrap(Wrap { trim: false }),
            content,
        );
    }
    let status = match (&w.error, w.finishing) {
        (Some(e), _) => Line::styled(e.clone(), Style::new().fg(theme.p.red)),
        (None, true) => Line::styled("Saving your profile…", theme.muted()),
        (None, false) => Line::styled(
            "Ctrl-C quits; answers saved so far are kept and onboarding resumes next time.",
            theme.dim(),
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
            .draw(|f| render(f, w, unicode, true, &Theme::new(Palette::TRUECOLOR)))
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

    fn sized(w: &Onboarding, unicode: bool, motion: bool, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| render(f, w, unicode, motion, &Theme::new(Palette::TRUECOLOR)))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .chunks(usize::from(width))
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn welcome_presents_the_observer_wordmark_and_next_step() {
        let mut w = wizard();
        let wide = sized(&w, true, true, 110, 32);
        assert!(wide.contains("█▀▀"), "wordmark\n{wide}");
        assert!(wide.contains(crate::ui::startup::TAGLINE));
        assert!(wide.contains("begin setup"));
        assert!(wide.contains('◉'), "the Observer's open eye\n{wide}");
        // The Observer turns with the clock, and only with motion.
        let first = sized(&w, true, true, 110, 32);
        w.tick = 20;
        assert_ne!(first, sized(&w, true, true, 110, 32), "it rotates");
        assert_eq!(
            sized(&w, true, false, 110, 32),
            sized(&wizard(), true, false, 110, 32)
        );
        let narrow = sized(&wizard(), true, true, 60, 30);
        assert!(
            narrow.contains("The Observer is awake"),
            "text survives\n{narrow}"
        );
        assert!(!narrow.contains("█▀▀"));
        assert!(sized(&wizard(), false, true, 110, 32).is_ascii());
    }

    #[test]
    fn the_stepper_names_every_page_and_marks_the_current_one() {
        let mut w = wizard();
        w.page = Page::Location;
        let s = sized(&w, true, true, 110, 32);
        for name in [
            "Welcome",
            "GitHub",
            "Location",
            "Where to look",
            "Interests",
            "Review",
        ] {
            assert!(s.contains(name), "{name}");
        }
        assert!(s.contains("step 3 of 6"));
        let small = sized(&w, true, true, 70, 30);
        assert!(small.contains("● ● ● ○ ○ ○"), "dots when narrow\n{small}");
    }
}
