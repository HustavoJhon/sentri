//! Draws the Observer awakening: the rotating ward, the SENTRI wordmark and
//! a panel with the real startup steps, location and discovery scope.
//!
//! Three layouts keep the screen balanced at any size: the ward above the
//! text when the terminal is tall, beside it when it is wide, and the small
//! flat ward (or just the steps) when space is short.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::observer::render::{self, Canvas, Cell};
use crate::observer::{self, Eye, Part};
use crate::state::startup::{Startup, Status};
use crate::theme::Theme;

pub const TAGLINE: &str = "THE WORLD IS FULL OF OPPORTUNITIES";
/// Inner width of the initialization panel.
const PANEL_WIDTH: u16 = 66;
/// Panel height: 5 steps, a rule, location, discovery and progress, plus
/// the border.
const PANEL_HEIGHT: u16 = 11;
/// Smallest 3D ward worth drawing; below it the flat ward is clearer.
const MIN_ART_ROWS: u16 = 14;
/// Columns per row the 3D ward needs to keep its proportions.
const ART_RATIO: f32 = 2.5;

/// Colors for the four brightness levels of each part of the 3D ward.
fn level_colors(part: Part, theme: &Theme) -> [Color; 4] {
    let p = &theme.p;
    match part {
        Part::Shell => [p.bg3, p.gray, p.yellow, p.fg],
        Part::Wing => [p.bg3, p.green, p.aqua, p.fg],
        Part::Staff => [p.bg3, p.gray, p.orange, p.yellow],
        Part::Core => [p.green, p.aqua, p.fg, p.yellow],
        Part::Base => [p.bg2, p.bg3, p.gray, p.fg4],
        Part::Eye => [p.gray, p.yellow, p.orange, p.yellow],
        Part::Pupil => [p.yellow, p.yellow, p.fg, p.fg],
        Part::Ring => [p.bg3, p.green, p.aqua, p.yellow],
        Part::Radar => [p.bg2, p.bg3, p.green, p.aqua],
        Part::Particle | Part::Spark | Part::Ray => [p.bg3, p.gray, p.fg4, p.yellow],
        Part::Blank => [p.bg0; 4],
    }
}

fn cell_style(cell: Cell, theme: &Theme) -> Style {
    let color = level_colors(cell.part, theme)[usize::from(cell.level.min(3))];
    let style = Style::new().fg(color).bg(theme.p.bg0);
    let bold = matches!(cell.part, Part::Pupil | Part::Eye) && cell.level >= 2
        || cell.part == Part::Ring && cell.level == 3;
    if bold {
        style.add_modifier(Modifier::BOLD)
    } else {
        style
    }
}

/// Copies a rendered frame into the buffer at `area`.
fn blit(buf: &mut Buffer, area: Rect, canvas: &Canvas, theme: &Theme) {
    for y in 0..canvas.height.min(usize::from(area.height)) {
        for x in 0..canvas.width.min(usize::from(area.width)) {
            let Some(cell) = canvas.get(x, y) else {
                continue;
            };
            let (Ok(dx), Ok(dy)) = (u16::try_from(x), u16::try_from(y)) else {
                continue;
            };
            let target = &mut buf[(area.x + dx, area.y + dy)];
            target.set_char(cell.ch);
            target.set_style(cell_style(cell, theme));
        }
    }
}

/// Style for the flat ward used on small terminals.
fn flat_style(part: Part, eye: Eye, tick: u64, theme: &Theme) -> Style {
    let p = &theme.p;
    let awake = eye >= Eye::Half;
    let style = match part {
        Part::Spark => Style::new().fg(p.yellow).add_modifier(Modifier::BOLD),
        Part::Ray => Style::new().fg(if tick % 6 < 3 { p.yellow } else { p.orange }),
        Part::Shell => Style::new().fg(if awake { p.yellow } else { p.bg3 }),
        Part::Wing => Style::new().fg(if awake { p.aqua } else { p.bg2 }),
        Part::Eye | Part::Pupil => match eye {
            Eye::Open => Style::new().fg(p.orange).add_modifier(Modifier::BOLD),
            Eye::Half => Style::new().fg(p.yellow),
            _ => Style::new().fg(p.gray),
        },
        Part::Staff => Style::new().fg(if awake { p.orange } else { p.bg3 }),
        Part::Base => Style::new().fg(p.bg2),
        Part::Blank | Part::Core | Part::Ring | Part::Radar | Part::Particle => Style::new(),
    };
    style.bg(p.bg0)
}

fn flat_ward(startup: &Startup, theme: &Theme) -> Vec<Line<'static>> {
    let eye = startup.display_eye();
    observer::frame(eye, startup.tick, startup.unicode)
        .into_iter()
        .map(|segments| {
            Line::from(
                segments
                    .into_iter()
                    .map(|(part, text)| {
                        Span::styled(text, flat_style(part, eye, startup.tick, theme))
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

/// Block letters for the wordmark, three rows each.
const LETTERS: [[&str; 3]; 6] = [
    ["█▀▀", "▀▀█", "▀▀▀"],
    ["█▀▀", "█▀▀", "▀▀▀"],
    ["█▄ █", "█ ▀█", "▀  ▀"],
    ["▀█▀", " █ ", " ▀ "],
    ["█▀▄", "█▀▄", "▀ ▀"],
    ["█", "█", "▀"],
];

/// The SENTRI wordmark: block letters warming from yellow to orange, or
/// spaced capitals without Unicode.
fn wordmark(unicode: bool, theme: &Theme) -> Vec<Line<'static>> {
    let p = &theme.p;
    let colors = [p.yellow, p.yellow, p.yellow, p.orange, p.orange, p.orange];
    if !unicode {
        let spans: Vec<Span> = "SENTRI"
            .chars()
            .zip(colors)
            .map(|(c, color)| {
                Span::styled(
                    format!("{c}  "),
                    Style::new().fg(color).add_modifier(Modifier::BOLD),
                )
            })
            .collect();
        return vec![Line::from(spans)];
    }
    (0..3)
        .map(|row| {
            let spans: Vec<Span> = LETTERS
                .iter()
                .zip(colors)
                .flat_map(|(letter, color)| {
                    [
                        Span::styled(
                            letter[row],
                            Style::new().fg(color).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                    ]
                })
                .collect();
            Line::from(spans)
        })
        .collect()
}

fn wordmark_width(unicode: bool) -> u16 {
    if unicode {
        // Letters plus one space after each.
        LETTERS
            .iter()
            .map(|l| u16::try_from(l[0].chars().count()).unwrap_or(0) + 1)
            .sum()
    } else {
        18
    }
}

fn tagline(width: u16, unicode: bool, theme: &Theme) -> Line<'static> {
    let text = u16::try_from(TAGLINE.len()).unwrap_or(u16::MAX);
    let rule = if unicode { "─" } else { "-" };
    let mut spans = Vec::new();
    // Short rules frame the tagline when there is room.
    let framed = width >= text + 12;
    if framed {
        spans.push(Span::styled(format!("{} ", rule.repeat(4)), theme.dim()));
    }
    spans.push(Span::styled(TAGLINE, Style::new().fg(theme.p.fg4)));
    if framed {
        spans.push(Span::styled(format!(" {}", rule.repeat(4)), theme.dim()));
    }
    Line::from(spans)
}

/// Spinner frame for a pending step while animated.
fn spinner(tick: u64, unicode: bool) -> &'static str {
    const BRAILLE: [&str; 8] = ["⠋", "⠙", "⠸", "⠴", "⠦", "⠇", "⠏", "⠹"];
    const ASCII: [&str; 4] = ["|", "/", "-", "\\"];
    let i = usize::try_from(tick / 2).unwrap_or(0);
    if unicode {
        BRAILLE[i % BRAILLE.len()]
    } else {
        ASCII[i % ASCII.len()]
    }
}

fn step_line<'a>(label: &'a str, status: &'a Status, startup: &Startup, theme: &Theme) -> Line<'a> {
    let unicode = startup.unicode;
    let pending_mark = if startup.animated() {
        spinner(startup.tick, unicode)
    } else if unicode {
        "◌"
    } else {
        "."
    };
    let (mark, mark_style, detail) = match status {
        Status::Pending => (pending_mark, Style::new().fg(theme.p.yellow), ""),
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

/// Location, discovery scope and overall progress, from real state.
fn context_lines(startup: &Startup, theme: &Theme) -> Vec<Line<'static>> {
    let c = &startup.context;
    let u = startup.unicode;
    let key = |icon_u: &'static str, icon_a: &'static str, name: &'static str| {
        vec![
            Span::styled(
                format!(" {} ", if u { icon_u } else { icon_a }),
                Style::new().fg(theme.p.aqua),
            ),
            Span::styled(format!("{name:<11}"), theme.muted()),
        ]
    };
    let mut location = key("◎", "o", "Location");
    match (&c.location, c.first_run) {
        (Some(place), _) => location.push(Span::styled(place.clone(), Style::new().fg(theme.p.fg))),
        (None, true) => location.push(Span::styled(
            "not set yet · onboarding opens next",
            theme.dim(),
        )),
        (None, false) => location.push(Span::styled("not set · Settings › Profile", theme.dim())),
    }
    let mut discovery = key("◈", "*", "Discovery");
    if c.scopes.is_empty() {
        discovery.push(Span::styled("no location bonus", theme.dim()));
    } else {
        discovery.push(Span::styled(
            c.scopes.join(" · "),
            Style::new().fg(theme.p.green),
        ));
    }
    if c.topics > 0 {
        let s = if c.topics == 1 { "" } else { "s" };
        discovery.push(Span::styled(
            format!("  {} topic{s}", c.topics),
            theme.muted(),
        ));
    }
    let done = startup.resolved();
    let total = startup.steps.len();
    let (full, empty) = if u { ("▰", "▱") } else { ("#", "-") };
    let progress = vec![
        Span::styled(
            format!(" {}", full.repeat(done)),
            Style::new().fg(theme.p.yellow),
        ),
        Span::styled(empty.repeat(total - done), theme.dim()),
        Span::styled(
            format!(
                "  {done}/{total} {}",
                if done == total {
                    "ready"
                } else {
                    "initializing"
                }
            ),
            theme.muted(),
        ),
    ];
    vec![
        Line::from(location),
        Line::from(discovery),
        Line::from(progress),
    ]
}

fn panel(frame: &mut Frame, area: Rect, startup: &Startup, theme: &Theme) {
    let rule = if startup.unicode { '─' } else { '-' };
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.p.bg3))
        .title(Span::styled(" INITIALIZATION ", theme.title()))
        .style(theme.base());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines: Vec<Line> = startup
        .steps
        .iter()
        .map(|(step, status)| step_line(step.label(), status, startup, theme))
        .collect();
    lines.push(Line::styled(
        rule.to_string().repeat(usize::from(inner.width)),
        Style::new().fg(theme.p.bg2),
    ));
    lines.extend(context_lines(startup, theme));
    frame.render_widget(Paragraph::new(lines), inner);
}

fn hint(theme: &Theme) -> Line<'static> {
    Line::styled(
        "any key skips · work continues in the background",
        theme.muted(),
    )
}

fn centered(r: Rect, w: u16, h: u16) -> Rect {
    let [col] = Layout::horizontal([Constraint::Length(w.min(r.width))])
        .flex(Flex::Center)
        .areas(r);
    let [cell] = Layout::vertical([Constraint::Length(h.min(r.height))])
        .flex(Flex::Center)
        .areas(col);
    cell
}

/// How the screen is arranged for a terminal size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arrangement {
    /// 3D ward of `rows` above the text.
    Stacked { rows: u16 },
    /// 3D ward of `rows` beside the text.
    Side { rows: u16 },
    /// Flat ward above the steps.
    Flat,
    /// Steps only.
    Steps,
}

/// Rows of wordmark, tagline, panel, hint and the gaps between them.
const fn text_height(unicode: bool) -> u16 {
    let mark = if unicode { 3 } else { 1 };
    mark + 1 + 1 + 1 + PANEL_HEIGHT + 1 + 1
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn arrange(area: Rect, unicode: bool) -> Arrangement {
    let rows_for = |cols: u16| (f32::from(cols) / ART_RATIO) as u16;
    let text = text_height(unicode);
    let panel = PANEL_WIDTH + 2;
    let stacked = area
        .height
        .saturating_sub(text + 1)
        .min(rows_for(area.width.saturating_sub(2)))
        .min(30);
    let side = area
        .height
        .saturating_sub(2)
        .min(rows_for(area.width.saturating_sub(panel + 6)))
        .min(34);
    let side_fits = area.height >= text && area.width >= panel + 6;
    if side_fits && side >= MIN_ART_ROWS && side > stacked + 2 {
        Arrangement::Side { rows: side }
    } else if stacked >= MIN_ART_ROWS && area.width >= panel {
        Arrangement::Stacked { rows: stacked }
    } else {
        let flat = u16::try_from(observer::HEIGHT).unwrap_or(11);
        if area.height >= flat + 2 + PANEL_HEIGHT && area.width >= 40 {
            Arrangement::Flat
        } else {
            Arrangement::Steps
        }
    }
}

/// Draws the wordmark, tagline, panel and hint into `area`, centered.
fn text_column(frame: &mut Frame, area: Rect, startup: &Startup, theme: &Theme) {
    let unicode = startup.unicode;
    let mark_rows = if unicode { 3 } else { 1 };
    let width = (PANEL_WIDTH + 2).min(area.width);
    let column = centered(area, width, text_height(unicode));
    let parts = Layout::vertical([
        Constraint::Length(mark_rows),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(PANEL_HEIGHT),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(column);
    frame.render_widget(
        Paragraph::new(wordmark(unicode, theme)),
        centered(parts[0], wordmark_width(unicode), mark_rows),
    );
    frame.render_widget(
        Paragraph::new(tagline(column.width, unicode, theme)).centered(),
        parts[2],
    );
    panel(frame, parts[4], startup, theme);
    frame.render_widget(Paragraph::new(hint(theme)).centered(), parts[6]);
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn ward_3d(frame: &mut Frame, area: Rect, rows: u16, startup: &Startup, theme: &Theme) {
    let cols = ((f32::from(rows) * ART_RATIO) as u16).min(area.width);
    let target = centered(area, cols, rows);
    let scene = observer::scene(
        startup.display_eye(),
        startup.tick,
        startup.glow,
        startup.animated(),
        startup.unicode,
    );
    let canvas = render::render(
        &scene,
        usize::from(target.width),
        usize::from(target.height),
    );
    blit(frame.buffer_mut(), target, &canvas, theme);
}

/// Draws the whole screen.
pub fn render(frame: &mut Frame, startup: &Startup, theme: &Theme) {
    let area = frame.area();
    frame.render_widget(Block::new().style(theme.base()), area);
    match arrange(area, startup.unicode) {
        Arrangement::Stacked { rows } => {
            let text = text_height(startup.unicode);
            let [art, rest] =
                Layout::vertical([Constraint::Length(rows + 1), Constraint::Length(text)])
                    .flex(Flex::Center)
                    .areas(area);
            ward_3d(frame, art, rows, startup, theme);
            text_column(frame, rest, startup, theme);
        }
        Arrangement::Side { rows } => {
            let [art, rest] =
                Layout::horizontal([Constraint::Fill(1), Constraint::Length(PANEL_WIDTH + 4)])
                    .areas(area.inner(ratatui::layout::Margin::new(1, 0)));
            ward_3d(frame, art, rows, startup, theme);
            text_column(frame, rest, startup, theme);
        }
        Arrangement::Flat => {
            let flat = u16::try_from(observer::HEIGHT).unwrap_or(11);
            let width = u16::try_from(observer::WIDTH).unwrap_or(25);
            let [art, title, steps] = Layout::vertical([
                Constraint::Length(flat + 1),
                Constraint::Length(1),
                Constraint::Length(PANEL_HEIGHT),
            ])
            .flex(Flex::Center)
            .areas(area);
            frame.render_widget(
                Paragraph::new(flat_ward(startup, theme)),
                centered(art, width, flat),
            );
            frame.render_widget(Paragraph::new(wordmark(false, theme)).centered(), title);
            panel(
                frame,
                centered(steps, PANEL_WIDTH + 2, PANEL_HEIGHT),
                startup,
                theme,
            );
        }
        Arrangement::Steps => {
            let [title, steps] =
                Layout::vertical([Constraint::Length(1), Constraint::Length(PANEL_HEIGHT)])
                    .flex(Flex::Center)
                    .areas(area);
            frame.render_widget(Paragraph::new(wordmark(false, theme)).centered(), title);
            panel(
                frame,
                centered(steps, PANEL_WIDTH + 2, PANEL_HEIGHT),
                startup,
                theme,
            );
        }
    }
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
    use crate::state::startup::{Context, Step};
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

    fn sample(mode: SplashMode, unicode: bool) -> Startup {
        let now = Instant::now();
        let mut s = Startup::new(mode, unicode, now);
        s.resolve(Step::Observer, Status::Done("Gruvbox Dark".into()), now);
        s.resolve(
            Step::Profile,
            Status::Background("GitHub later".into()),
            now,
        );
        s.context = Context {
            location: Some("Cusco, Peru".into()),
            scopes: vec!["local", "national"],
            topics: 2,
            first_run: false,
        };
        s
    }

    #[test]
    fn shows_every_step_with_its_real_state() {
        let s = sample(SplashMode::Static, true);
        let screen = text(100, 44, &s);
        for step in Step::ALL {
            assert!(screen.contains(step.label()), "{}", step.label());
        }
        assert!(screen.contains("✓ Initializing Observer"));
        assert!(screen.contains("↻ Loading personal profile"));
        assert!(screen.contains("◌ Preparing dashboard"));
        assert!(screen.contains("2/5 initializing"), "{screen}");
        assert!(screen.contains(TAGLINE));
        assert!(screen.contains("Cusco, Peru"));
        assert!(screen.contains("local · national"));
        assert!(screen.contains("2 topics"));
        assert!(screen.contains("█▀▀"), "the wordmark is drawn");
    }

    #[test]
    fn nothing_is_claimed_without_configuration() {
        let now = Instant::now();
        let mut s = Startup::new(SplashMode::Static, true, now);
        s.context.first_run = true;
        let screen = text(100, 44, &s);
        assert!(screen.contains("0/5 initializing"));
        assert!(!screen.contains('✓'), "no step is marked done");
        assert!(screen.contains("onboarding opens next"));
    }

    #[test]
    fn arrangement_follows_the_terminal_size() {
        let r = |w, h| arrange(Rect::new(0, 0, w, h), true);
        assert!(matches!(r(100, 44), Arrangement::Stacked { rows } if rows >= 20));
        assert!(matches!(r(160, 30), Arrangement::Side { rows } if rows >= 20));
        assert_eq!(r(80, 24), Arrangement::Flat);
        assert_eq!(r(50, 14), Arrangement::Steps);
        assert_eq!(r(30, 8), Arrangement::Steps);
    }

    #[test]
    fn draws_at_every_size_without_panicking() {
        for unicode in [true, false] {
            for mode in [SplashMode::Static, SplashMode::Animated] {
                let mut s = sample(mode, unicode);
                for (w, h) in [
                    (1, 1),
                    (20, 5),
                    (60, 20),
                    (80, 24),
                    (120, 40),
                    (200, 60),
                    (160, 30),
                ] {
                    s.on_timer(Instant::now());
                    let screen = text(w, h, &s);
                    if !unicode {
                        let ward: String = screen.chars().filter(|c| !c.is_ascii()).collect();
                        assert!(ward.is_empty(), "{w}x{h}: {ward:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_3d_ward_fills_large_terminals() {
        let s = sample(SplashMode::Static, true);
        let screen = text(120, 48, &s);
        let ward_rows = screen
            .lines()
            .take_while(|l| !l.contains('█'))
            .filter(|l| !l.trim().is_empty())
            .count();
        assert!(ward_rows >= 20, "{ward_rows}\n{screen}");
    }

    #[test]
    fn ascii_fallback_and_tiny_terminals() {
        let s = sample(SplashMode::Static, false);
        let screen = text(100, 44, &s);
        assert!(screen.contains("S  E  N  T  R  I"));
        assert!(screen.contains("+ Initializing Observer"));
        let flat = text(80, 24, &s);
        assert!(flat.contains("<##--|"), "the flat ward on small screens");
        let small = text(50, 12, &s);
        assert!(small.contains("Preparing dashboard"), "steps survive");
        assert!(!small.contains("<##"), "the drawing is dropped");
    }
}
