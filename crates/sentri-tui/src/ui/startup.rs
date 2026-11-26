//! Draws the Observer awakening as an observation console: the rotating ward
//! and the SENTRI wordmark in the centre, the real initialization steps on
//! the left, an event log on the right and a status bar along the bottom.
//!
//! Side panels sit on the darker Gruvbox background instead of boxes, so the
//! screen has depth without a frame around everything. Smaller terminals
//! drop the log, then shrink to the flat ward, then to the steps alone.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};

use crate::observer::render::{self, Canvas, Cell};
use crate::observer::{self, Eye, Part};
use crate::state::startup::{Level, Startup, Status};
use crate::theme::Theme;

pub const TAGLINE: &str = "THE WORLD IS FULL OF OPPORTUNITIES";
/// Smallest 3D ward worth drawing; below it the flat ward is clearer.
const MIN_ART_ROWS: u16 = 11;
/// Columns per row the 3D ward needs to keep its proportions.
const ART_RATIO: f32 = 2.5;

/// Colors for the four brightness levels of each part of the 3D ward.
fn level_colors(part: Part, theme: &Theme) -> [Color; 4] {
    let p = &theme.p;
    match part {
        Part::Shell => [p.bg3, p.gray, p.yellow, p.fg],
        Part::Wing => [p.bg3, p.green, p.aqua, p.fg],
        Part::Staff => [p.bg3, p.orange, p.orange, p.yellow],
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
pub(super) fn blit(buf: &mut Buffer, area: Rect, canvas: &Canvas, theme: &Theme) {
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
pub(super) fn wordmark(unicode: bool, theme: &Theme) -> Vec<Line<'static>> {
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

pub(super) fn wordmark_width(unicode: bool) -> u16 {
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

pub(super) fn tagline(width: u16, unicode: bool, theme: &Theme) -> Line<'static> {
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

/// Width of the initialization column, and of the event log.
const SIDE: u16 = 34;
/// Width of the initialization column when the log is hidden.
const SIDE_NARROW: u16 = 30;
/// Rows of the bottom status bar.
const BAR: u16 = 2;

fn mark(status: &Status, startup: &Startup, theme: &Theme) -> (&'static str, Style) {
    let u = startup.unicode;
    match status {
        Status::Pending => (
            if startup.animated() {
                spinner(startup.tick, u)
            } else if u {
                "◌"
            } else {
                "."
            },
            Style::new().fg(theme.p.yellow),
        ),
        Status::Done(_) => (if u { "✓" } else { "+" }, Style::new().fg(theme.p.green)),
        Status::Background(_) => (if u { "↻" } else { "~" }, Style::new().fg(theme.p.blue)),
        Status::Failed(_) => (if u { "✗" } else { "x" }, Style::new().fg(theme.p.red)),
    }
}

fn detail(status: &Status) -> &str {
    match status {
        Status::Pending => "",
        Status::Done(d) | Status::Background(d) | Status::Failed(d) => d,
    }
}

/// Cuts `text` to `width` columns, with an ellipsis when it is longer.
fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(width.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn heading(title: &'static str, width: u16, unicode: bool, theme: &Theme) -> Vec<Line<'static>> {
    let rule = if unicode { "─" } else { "-" };
    vec![
        Line::from(vec![
            Span::styled(
                if unicode { "▍" } else { "|" },
                Style::new().fg(theme.p.orange),
            ),
            Span::styled(title, theme.title()),
        ]),
        Line::styled(
            rule.repeat(usize::from(width)),
            Style::new().fg(theme.p.bg2),
        ),
    ]
}

/// The initialization column: every step, with what it found below it when
/// there is room.
fn steps_column(frame: &mut Frame, area: Rect, startup: &Startup, theme: &Theme) {
    let width = area.width.saturating_sub(2);
    let mut lines = heading(" INITIALIZATION", width, startup.unicode, theme);
    let roomy = area.height > 2 + 2 * u16::try_from(startup.steps.len()).unwrap_or(6);
    for (step, status) in &startup.steps {
        let (m, style) = mark(status, startup, theme);
        let label = if status.is_pending() {
            theme.dim()
        } else {
            Style::new().fg(theme.p.fg)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{m} "), style),
            Span::styled(step.label(), label),
        ]));
        if roomy {
            let text = detail(status);
            let shown = if text.is_empty() { "…" } else { text };
            lines.push(Line::styled(
                format!("  {}", fit(shown, usize::from(width).saturating_sub(2))),
                theme.muted(),
            ));
        }
    }
    frame.render_widget(
        Paragraph::new(lines).style(Style::new().bg(theme.p.bg0_h)),
        area.inner(ratatui::layout::Margin::new(1, 0)),
    );
}

fn level_style(level: Level, theme: &Theme) -> Style {
    Style::new().fg(match level {
        Level::Info => theme.p.aqua,
        Level::Done => theme.p.green,
        Level::Background => theme.p.blue,
        Level::Failed => theme.p.red,
    })
}

/// The event log: newest at the bottom; the latest line types itself in.
fn log_column(frame: &mut Frame, area: Rect, startup: &Startup, theme: &Theme) {
    let inner = area.inner(ratatui::layout::Margin::new(1, 0));
    let width = usize::from(inner.width);
    let mut body: Vec<Line> = Vec::new();
    let last = startup.events.len().saturating_sub(1);
    for (i, e) in startup.events.iter().enumerate() {
        let secs = e.at.as_secs_f32();
        body.push(Line::from(vec![
            Span::styled(format!("{secs:>5.2}s "), theme.dim()),
            Span::styled(
                e.tag,
                level_style(e.level, theme).add_modifier(Modifier::BOLD),
            ),
        ]));
        let mut text = e.text.clone();
        if i == last && startup.animated() {
            // About 90 characters per second: a typing effect, nothing more.
            let since = startup.clock.saturating_sub(e.at).as_millis();
            let shown = usize::try_from(since / 11).unwrap_or(usize::MAX);
            text = text.chars().take(shown.max(1)).collect();
        }
        for chunk in wrap(&text, width.saturating_sub(2).max(8)) {
            body.push(Line::styled(
                format!("  {chunk}"),
                Style::new().fg(theme.p.fg4),
            ));
        }
    }
    let mut lines = heading(" EVENT LOG", inner.width, startup.unicode, theme);
    let room = usize::from(inner.height).saturating_sub(lines.len());
    let skip = body.len().saturating_sub(room);
    lines.extend(body.into_iter().skip(skip));
    frame.render_widget(
        Paragraph::new(lines).style(Style::new().bg(theme.p.bg0_h)),
        inner,
    );
}

/// Splits `text` into lines of at most `width` columns, at spaces.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let needed = line.chars().count() + usize::from(!line.is_empty()) + word.chars().count();
        if !line.is_empty() && needed > width {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() || out.is_empty() {
        out.push(line);
    }
    out.into_iter().map(|l| fit(&l, width)).collect()
}

/// Status bar: progress, location, discovery scope and the skip hint.
fn status_bar(frame: &mut Frame, area: Rect, startup: &Startup, theme: &Theme) {
    let u = startup.unicode;
    let c = &startup.context;
    let done = startup.resolved();
    let total = startup.steps.len();
    let (full, empty) = if u { ("▰", "▱") } else { ("#", "-") };
    let label = |t: &'static str| Span::styled(t, theme.muted());
    let mut top = vec![
        label(" SYSTEM "),
        Span::styled(full.repeat(done), Style::new().fg(theme.p.yellow)),
        Span::styled(empty.repeat(total - done), Style::new().fg(theme.p.bg3)),
        Span::styled(
            format!(
                " {done}/{total} {}",
                if done == total {
                    "ready"
                } else {
                    "initializing"
                }
            ),
            Style::new().fg(if done == total {
                theme.p.green
            } else {
                theme.p.fg
            }),
        ),
        label("    LOCATION "),
    ];
    match (&c.location, c.first_run) {
        (Some(place), _) => top.push(Span::styled(place.clone(), Style::new().fg(theme.p.fg))),
        (None, true) => top.push(Span::styled("not set · onboarding next", theme.dim())),
        (None, false) => top.push(Span::styled("not set", theme.dim())),
    }
    top.push(label("    DISCOVERY "));
    let (on, off) = if u { ("●", "○") } else { ("*", "o") };
    for (name, scope) in [
        ("local", "local"),
        ("national", "national"),
        ("global", "international"),
    ] {
        let enabled = c.scopes.contains(&scope);
        top.push(Span::styled(
            format!("{} ", if enabled { on } else { off }),
            Style::new().fg(if enabled { theme.p.green } else { theme.p.bg3 }),
        ));
        top.push(Span::styled(
            format!("{name}  "),
            if enabled {
                Style::new().fg(theme.p.fg)
            } else {
                theme.dim()
            },
        ));
    }
    let secs = startup.clock.as_secs_f32();
    let bottom = Line::from(vec![
        Span::styled(format!(" T+{secs:.1}s"), theme.dim()),
        Span::styled(
            "   any key skips · work continues in the background",
            theme.muted(),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(vec![Line::from(top), bottom]).style(Style::new().bg(theme.p.bg0_h)),
        area,
    );
}

/// One line across the top: name and version, and when nothing is ready
/// yet, the state of the Observer.
fn header(frame: &mut Frame, area: Rect, startup: &Startup, theme: &Theme) {
    let eye = startup.display_eye();
    let state = match eye {
        Eye::Closed => "dormant",
        Eye::Stirring => "stirring",
        Eye::Half => "waking",
        Eye::Open => "awake · scanning",
    };
    let glyph = if startup.unicode { "◉" } else { "@" };
    let line = Line::from(vec![
        Span::styled(format!(" {glyph} "), Style::new().fg(theme.p.orange)),
        Span::styled("SENTRI", theme.title()),
        Span::styled(" · OBSERVER CONSOLE", theme.muted()),
        Span::styled(format!(" · v{}", env!("CARGO_PKG_VERSION")), theme.dim()),
        Span::styled("    observer ", theme.muted()),
        Span::styled(
            state,
            Style::new().fg(if eye == Eye::Open {
                theme.p.green
            } else {
                theme.p.yellow
            }),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::new().bg(theme.p.bg0_h)),
        area,
    );
}

pub(super) fn centered(r: Rect, w: u16, h: u16) -> Rect {
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
    /// Steps, ward and event log side by side.
    Console,
    /// Steps and ward; no log.
    Duo,
    /// Flat ward above a compact list of steps.
    Flat,
    /// Steps only.
    Steps,
}

/// Rows the centre needs besides the ward: wordmark, gap and tagline.
const fn lockup(unicode: bool) -> u16 {
    if unicode { 5 } else { 3 }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn art_rows(center: Rect, unicode: bool) -> u16 {
    let by_width = (f32::from(center.width.saturating_sub(2)) / ART_RATIO) as u16;
    center
        .height
        .saturating_sub(lockup(unicode) + 1)
        .min(by_width)
        .min(34)
}

fn arrange(area: Rect, unicode: bool) -> Arrangement {
    let body = |side: u16| {
        Rect::new(
            0,
            0,
            area.width.saturating_sub(side),
            area.height.saturating_sub(1 + BAR),
        )
    };
    if area.width >= 118 && area.height >= 28 && art_rows(body(2 * SIDE + 2), unicode) >= 16 {
        Arrangement::Console
    } else if area.width >= 76
        && area.height >= 22
        && art_rows(body(SIDE_NARROW + 1), unicode) >= MIN_ART_ROWS
    {
        Arrangement::Duo
    } else if area.width >= 44 && area.height >= 21 {
        Arrangement::Flat
    } else {
        Arrangement::Steps
    }
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

/// The ward with the wordmark and tagline beneath it.
fn centre(frame: &mut Frame, area: Rect, startup: &Startup, theme: &Theme) {
    let unicode = startup.unicode;
    let rows = art_rows(area, unicode);
    let mark_rows = if unicode { 3 } else { 1 };
    let [art, mark, _, tag] = Layout::vertical([
        Constraint::Length(rows),
        Constraint::Length(mark_rows),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .flex(Flex::Center)
    .areas(area);
    ward_3d(frame, art, rows, startup, theme);
    frame.render_widget(
        Paragraph::new(wordmark(unicode, theme)),
        centered(mark, wordmark_width(unicode), mark_rows),
    );
    frame.render_widget(
        Paragraph::new(tagline(area.width, unicode, theme)).centered(),
        tag,
    );
}

/// Steps as single lines, for small terminals.
fn compact_steps(startup: &Startup, theme: &Theme) -> Vec<Line<'static>> {
    startup
        .steps
        .iter()
        .map(|(step, status)| {
            let (m, style) = mark(status, startup, theme);
            let label = if status.is_pending() {
                theme.dim()
            } else {
                Style::new().fg(theme.p.fg)
            };
            Line::from(vec![
                Span::styled(format!("{m} "), style),
                Span::styled(format!("{:<23}", step.label()), label),
                Span::styled(fit(detail(status), 28), theme.muted()),
            ])
        })
        .collect()
}

/// Draws the whole screen.
pub fn render(frame: &mut Frame, startup: &Startup, theme: &Theme) {
    let area = frame.area();
    frame.render_widget(Block::new().style(theme.base()), area);
    let arrangement = arrange(area, startup.unicode);
    match arrangement {
        Arrangement::Console | Arrangement::Duo => {
            let [top, body, bar] = Layout::vertical([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(BAR),
            ])
            .areas(area);
            header(frame, top, startup, theme);
            status_bar(frame, bar, startup, theme);
            if arrangement == Arrangement::Console {
                let [left, _, mid, _, right] = Layout::horizontal([
                    Constraint::Length(SIDE),
                    Constraint::Length(1),
                    Constraint::Min(0),
                    Constraint::Length(1),
                    Constraint::Length(SIDE),
                ])
                .areas(body);
                steps_column(frame, left, startup, theme);
                centre(frame, mid, startup, theme);
                log_column(frame, right, startup, theme);
            } else {
                let [left, _, mid] = Layout::horizontal([
                    Constraint::Length(SIDE_NARROW),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ])
                .areas(body);
                steps_column(frame, left, startup, theme);
                centre(frame, mid, startup, theme);
            }
        }
        Arrangement::Flat => {
            let flat = u16::try_from(observer::HEIGHT).unwrap_or(11);
            let width = u16::try_from(observer::WIDTH).unwrap_or(25);
            let steps = u16::try_from(startup.steps.len()).unwrap_or(6);
            let [art, title, list, _, bar] = Layout::vertical([
                Constraint::Length(flat),
                Constraint::Length(1),
                Constraint::Length(steps),
                Constraint::Length(1),
                Constraint::Length(BAR),
            ])
            .flex(Flex::Center)
            .areas(area);
            frame.render_widget(
                Paragraph::new(flat_ward(startup, theme)),
                centered(art, width, flat),
            );
            frame.render_widget(Paragraph::new(wordmark(false, theme)).centered(), title);
            frame.render_widget(
                Paragraph::new(compact_steps(startup, theme)),
                centered(list, 56, steps),
            );
            status_bar(frame, bar, startup, theme);
        }
        Arrangement::Steps => {
            let steps = u16::try_from(startup.steps.len()).unwrap_or(6);
            let [title, list, bar] = Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(steps),
                Constraint::Length(BAR),
            ])
            .flex(Flex::Center)
            .areas(area);
            frame.render_widget(Paragraph::new(wordmark(false, theme)).centered(), title);
            frame.render_widget(
                Paragraph::new(compact_steps(startup, theme)).wrap(Wrap { trim: true }),
                list,
            );
            status_bar(frame, bar, startup, theme);
        }
    }
    if !startup.unicode {
        super::ascii::asciify(frame.buffer_mut());
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

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
        s.log(now, Level::Info, "sources", "12 healthy · 1 failing".into());
        s.context = Context {
            location: Some("Cusco, Peru".into()),
            scopes: vec!["local", "national"],
            topics: 2,
            first_run: false,
        };
        s
    }

    #[test]
    fn the_console_shows_steps_log_and_status_from_real_state() {
        let s = sample(SplashMode::Static, true);
        let screen = text(140, 40, &s);
        for step in Step::ALL {
            assert!(screen.contains(step.label()), "{}", step.label());
        }
        assert!(screen.contains("✓ Initializing Observer"));
        assert!(screen.contains("↻ Loading user profile"));
        assert!(screen.contains("◌ Preparing dashboard"));
        assert!(screen.contains("2/6 initializing"), "{screen}");
        assert!(screen.contains("EVENT LOG"));
        assert!(
            screen.contains("12 healthy · 1 failing"),
            "logged facts appear"
        );
        assert!(screen.contains("GitHub later"), "step details appear");
        assert!(screen.contains(TAGLINE));
        assert!(screen.contains("Cusco, Peru"));
        assert!(screen.contains("● local") && screen.contains("○ global"));
        assert!(screen.contains("█▀▀"), "the wordmark is drawn");
        assert!(screen.contains("OBSERVER CONSOLE"));
    }

    #[test]
    fn nothing_is_claimed_without_configuration() {
        let now = Instant::now();
        let mut s = Startup::new(SplashMode::Static, true, now);
        s.context.first_run = true;
        let screen = text(140, 40, &s);
        assert!(screen.contains("0/6 initializing"));
        assert!(!screen.contains('✓'), "no step is marked done");
        assert!(screen.contains("onboarding next"));
        assert!(screen.contains("dormant"));
    }

    #[test]
    fn arrangement_follows_the_terminal_size() {
        let r = |w, h| arrange(Rect::new(0, 0, w, h), true);
        assert_eq!(r(140, 40), Arrangement::Console);
        assert_eq!(r(200, 60), Arrangement::Console);
        assert_eq!(r(100, 30), Arrangement::Duo);
        assert_eq!(r(80, 24), Arrangement::Duo);
        assert_eq!(r(60, 22), Arrangement::Flat);
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
                    (60, 22),
                    (80, 24),
                    (120, 40),
                    (200, 60),
                    (160, 30),
                ] {
                    s.on_timer(Instant::now());
                    let screen = text(w, h, &s);
                    if !unicode {
                        let odd: String = screen.chars().filter(|c| !c.is_ascii()).collect();
                        assert!(odd.is_empty(), "{w}x{h}: {odd:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_newest_event_types_itself_in() {
        let mut s = sample(SplashMode::Animated, true);
        let t0 = Instant::now();
        s.log(
            t0,
            Level::Done,
            "dashboard",
            "157 opportunities ranked".into(),
        );
        let at = s.events.last().unwrap().at;
        s.clock = at;
        assert!(!text(140, 40, &s).contains("157 opportunities ranked"));
        s.clock = at + Duration::from_secs(1);
        assert!(text(140, 40, &s).contains("157 opportunities ranked"));
    }

    #[test]
    fn the_log_keeps_the_newest_entries_when_full() {
        let mut s = sample(SplashMode::Static, true);
        let t0 = Instant::now();
        for i in 0..40 {
            s.log(t0, Level::Info, "sources", format!("entry {i:02}"));
        }
        let screen = text(140, 40, &s);
        assert!(screen.contains("entry 39"));
        assert!(!screen.contains("entry 00"));
    }

    #[test]
    fn wrap_breaks_at_spaces_and_fits() {
        assert_eq!(wrap("a bb ccc", 4), ["a bb", "ccc"]);
        assert_eq!(wrap("", 5), [""]);
        assert_eq!(wrap("abcdefgh", 4), ["abc…"]);
    }

    #[test]
    fn ascii_fallback_and_small_terminals() {
        let s = sample(SplashMode::Static, false);
        let screen = text(140, 40, &s);
        assert!(screen.contains("S  E  N  T  R  I"));
        assert!(screen.contains("+ Initializing Observer"));
        let flat = text(60, 22, &s);
        assert!(
            flat.contains("<##--|"),
            "the flat ward on small screens\n{flat}"
        );
        let small = text(50, 14, &s);
        assert!(small.contains("Preparing dashboard"), "steps survive");
        assert!(!small.contains("<##"), "the drawing is dropped");
    }
}
