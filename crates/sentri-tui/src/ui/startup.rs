//! Draws the Observer awakening: the ward and the real startup steps.

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::observer::{self, Eye, Part};
use crate::state::startup::{Startup, Status};
use crate::theme::Theme;

fn part_style(part: Part, eye: Eye, tick: u64, theme: &Theme) -> Style {
    let p = &theme.p;
    let awake = eye >= Eye::Half;
    let style = match part {
        Part::Blank => Style::new(),
        Part::Spark => Style::new().fg(p.yellow).add_modifier(Modifier::BOLD),
        Part::Ray => Style::new().fg(if tick % 6 < 3 { p.yellow } else { p.orange }),
        Part::Shell => Style::new().fg(if awake { p.yellow } else { p.bg3 }),
        Part::Wing => Style::new().fg(if awake { p.aqua } else { p.bg2 }),
        Part::Eye => match eye {
            Eye::Open => Style::new().fg(p.orange).add_modifier(Modifier::BOLD),
            Eye::Half => Style::new().fg(p.yellow),
            _ => Style::new().fg(p.gray),
        },
        Part::Staff => Style::new().fg(if awake { p.orange } else { p.bg3 }),
        Part::Base => Style::new().fg(p.bg2),
    };
    style.bg(p.bg0)
}

fn step_line<'a>(label: &'a str, status: &'a Status, unicode: bool, theme: &Theme) -> Line<'a> {
    let (mark, mark_style, detail) = match status {
        Status::Pending => (if unicode { "◌" } else { "." }, theme.muted(), ""),
        Status::Done(d) => (
            if unicode { "✓" } else { "+" },
            Style::new().fg(theme.p.green),
            d.as_str(),
        ),
        Status::Background(d) => (
            if unicode { "↻" } else { "~" },
            Style::new().fg(theme.p.blue),
            d.as_str(),
        ),
        Status::Failed(d) => (
            if unicode { "✗" } else { "x" },
            Style::new().fg(theme.p.red),
            d.as_str(),
        ),
    };
    let label_style = if status.is_pending() {
        theme.dim()
    } else {
        Style::new().fg(theme.p.fg)
    };
    let mut spans = vec![
        Span::styled(format!(" {mark} "), mark_style),
        Span::styled(format!("{label:<31}"), label_style),
    ];
    if !detail.is_empty() {
        spans.push(Span::styled(detail, theme.muted()));
    }
    Line::from(spans)
}

/// Draws the whole screen.
pub fn render(frame: &mut Frame, startup: &Startup, theme: &Theme) {
    let area = frame.area();
    frame.render_widget(Block::new().style(theme.base()), area);
    let eye = startup.eye();
    let art: Vec<Line> = observer::frame(eye, startup.tick, startup.unicode)
        .into_iter()
        .map(|segments| {
            Line::from(
                segments
                    .into_iter()
                    .map(|(part, text)| {
                        Span::styled(text, part_style(part, eye, startup.tick, theme))
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let steps: Vec<Line> = startup
        .steps
        .iter()
        .map(|(step, status)| step_line(step.label(), status, startup.unicode, theme))
        .collect();
    let title = Line::from(vec![
        Span::styled("S E N T R I", theme.title()),
        Span::styled(
            if eye == Eye::Open {
                "  ·  the Observer is awake"
            } else {
                "  ·  the Observer awakens"
            },
            theme.muted(),
        ),
    ]);
    let hint = Line::styled("any key skips · work continues", theme.muted());

    // Small terminals drop the drawing and keep the steps.
    let art_rows = u16::try_from(observer::HEIGHT).unwrap_or(11);
    let show_art = area.height >= art_rows + 10 && area.width >= 40;
    let mut rows = vec![Constraint::Length(1), Constraint::Length(1)];
    if show_art {
        rows.insert(0, Constraint::Length(art_rows + 1));
    }
    rows.push(Constraint::Length(5));
    rows.push(Constraint::Length(2));
    let height = rows
        .iter()
        .map(|c| match c {
            Constraint::Length(n) => *n,
            _ => 0,
        })
        .sum::<u16>();
    let [column] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    let parts = Layout::vertical(rows).split(column);
    let mut i = 0;
    if show_art {
        let width = u16::try_from(observer::WIDTH).unwrap_or(25);
        let [art_area] = Layout::horizontal([Constraint::Length(width)])
            .flex(Flex::Center)
            .areas(parts[0]);
        frame.render_widget(Paragraph::new(art), art_area);
        i = 1;
    }
    let centered = |r: Rect, w: u16| {
        let [a] = Layout::horizontal([Constraint::Length(w.min(r.width))])
            .flex(Flex::Center)
            .areas(r);
        a
    };
    frame.render_widget(
        Paragraph::new(title).centered(),
        centered(parts[i], area.width),
    );
    frame.render_widget(Paragraph::new(steps), centered(parts[i + 2], 72));
    frame.render_widget(
        Paragraph::new(hint).centered(),
        centered(parts[i + 3], area.width),
    );
    if !startup.unicode {
        super::ascii::asciify(frame.buffer_mut());
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::observer::SplashMode;
    use crate::state::startup::Step;
    use crate::theme::Palette;

    fn text(width: u16, height: u16, startup: &Startup) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| render(f, startup, &Theme::new(Palette::TRUECOLOR)))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        buffer
            .content()
            .chunks(width as usize)
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn shows_every_step_with_its_real_state() {
        let now = Instant::now();
        let mut s = Startup::new(SplashMode::Static, true, now);
        s.resolve(Step::Observer, Status::Done("Gruvbox Dark".into()), now);
        s.resolve(
            Step::Profile,
            Status::Background("GitHub later".into()),
            now,
        );
        let screen = text(90, 30, &s);
        for step in Step::ALL {
            assert!(screen.contains(step.label()), "{}", step.label());
        }
        assert!(screen.contains("✓ Initializing Observer"));
        assert!(screen.contains("↻ Loading personal profile"));
        assert!(screen.contains("◌ Preparing dashboard"));
        assert!(screen.contains('┃'), "the ward is drawn");
    }

    #[test]
    fn ascii_fallback_and_tiny_terminals() {
        let now = Instant::now();
        let s = Startup::new(SplashMode::Static, false, now);
        let screen = text(90, 30, &s);
        assert!(screen.contains("<##--|"));
        assert!(!screen.contains('┃'));
        let small = text(50, 12, &s);
        assert!(small.contains("Preparing dashboard"), "steps survive");
        assert!(!small.contains("<##"), "the drawing is dropped");
    }
}
