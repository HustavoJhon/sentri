//! Terminal lifecycle and the event loop.
//!
//! Terminal events are read on a dedicated thread; effects run as tokio tasks
//! and report back through a channel, so the UI never blocks on the network.

use std::io::stdout;
use std::time::Duration;

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind,
};
use ratatui::crossterm::execute;
use sentri_app::Sentri;
use sentri_core::{Action, FeedQuery};
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::state::{Effect, Msg, Overview, State};
use crate::theme::{Palette, Theme};
use crate::ui;

/// Errors that end the TUI.
#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("terminal error: {0}")]
    Io(#[from] std::io::Error),
}

/// Opens the TUI and runs until the user quits.
pub async fn run(app: Sentri, config_path: String) -> Result<(), TuiError> {
    let mut terminal = ratatui::init();
    // Mouse capture is best effort: some terminals do not support it.
    let mouse = execute!(stdout(), EnableMouseCapture).is_ok();
    let result = event_loop(&mut terminal, app, config_path).await;
    if mouse {
        let _ = execute!(stdout(), DisableMouseCapture);
    }
    ratatui::restore();
    result
}

async fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: Sentri,
    config_path: String,
) -> Result<(), TuiError> {
    let ui = &app.config().ui;
    let palette = match ui.colors {
        sentri_app::ColorMode::Auto => Palette::detect(),
        sentri_app::ColorMode::Truecolor => Palette::TRUECOLOR,
        sentri_app::ColorMode::Indexed => Palette::INDEXED,
    };
    let theme = Theme::new(palette);
    let (event_tx, mut events) = mpsc::unbounded_channel();
    spawn_terminal_reader(event_tx);
    let (msg_tx, mut msgs) = mpsc::unbounded_channel();

    let mut state = State {
        loading: true,
        density: crate::state::Density::from_name(&ui.density).unwrap_or_default(),
        ui: crate::state::UiPrefs::from_config(ui, palette == Palette::INDEXED),
        ..State::default()
    };
    execute(&app, &state, Effect::Load, &config_path, &msg_tx);

    while !state.quit {
        terminal.draw(|frame| ui::draw(frame, &mut state, &theme))?;
        let effects = tokio::select! {
            Some(event) = events.recv() => match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => state.on_key(key),
                Event::Mouse(mouse) => state.on_mouse(mouse),
                _ => Vec::new(), // resize: just redraw
            },
            Some(msg) = msgs.recv() => state.on_msg(msg),
            else => break,
        };
        let pending = state.pending();
        for effect in effects.into_iter().chain(pending) {
            execute(&app, &state, effect, &config_path, &msg_tx);
        }
    }
    Ok(())
}

fn spawn_terminal_reader(tx: UnboundedSender<Event>) {
    std::thread::spawn(move || {
        loop {
            match event::poll(Duration::from_millis(250)) {
                Ok(true) => match event::read() {
                    Ok(ev) => {
                        if tx.send(ev).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                },
                Ok(false) => {
                    if tx.is_closed() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

/// Runs an effect in the background and reports the result as a [`Msg`].
fn execute(
    app: &Sentri,
    state: &State,
    effect: Effect,
    config_path: &str,
    tx: &UnboundedSender<Msg>,
) {
    let app = app.clone();
    let tx = tx.clone();
    match effect {
        Effect::Load => {
            let query = state.effective_query();
            let config_path = config_path.to_owned();
            tokio::spawn(async move {
                let msg = load(&app, &query).await;
                let _ = tx.send(msg);
                let _ = tx.send(overview(&app, config_path).await);
            });
        }
        Effect::Act(id, action) => {
            tokio::spawn(async move {
                let msg = match app.act(id, action).await {
                    Ok(new_state) => Msg::Acted(id, new_state),
                    Err(e) => Msg::Failed(format!("Could not update #{id}: {e}")),
                };
                let _ = tx.send(msg);
                if !matches!(action, Action::Viewed) {
                    let _ = tx.send(Msg::Info(describe(action)));
                }
            });
        }
        Effect::AddLink(url) => {
            tokio::spawn(async move {
                let msg = match app
                    .add_link(&url, &sentri_app::LinkOverrides::default())
                    .await
                {
                    Ok(outcome) => Msg::Added(Box::new(outcome)),
                    Err(e) => Msg::Failed(format!("Could not add link: {e}")),
                };
                let _ = tx.send(msg);
            });
        }
        Effect::Collect => {
            tokio::spawn(async move {
                let reports = app.collect(None).await;
                let new: i64 = reports
                    .iter()
                    .filter_map(|r| r.result.as_ref().ok())
                    .map(|(_, n)| n)
                    .sum();
                let failed: Vec<&str> = reports
                    .iter()
                    .filter(|r| r.result.is_err())
                    .map(|r| r.source.as_str())
                    .collect();
                let mut text = format!("Collected from {} sources: {new} new", reports.len());
                if !failed.is_empty() {
                    text.push_str(" · failed: ");
                    text.push_str(&failed.join(", "));
                }
                let _ = tx.send(Msg::Collected(text));
            });
        }
        Effect::LoadDetail(id) => {
            tokio::spawn(async move {
                let msg = match (app.edit_state(id).await, app.history_for(id, 20).await) {
                    (Ok(edit), Ok(history)) => {
                        Msg::Detail(Box::new(crate::state::DetailData { id, edit, history }))
                    }
                    (Err(e), _) | (_, Err(e)) => Msg::Failed(format!("Could not load #{id}: {e}")),
                };
                let _ = tx.send(msg);
            });
        }
        Effect::LoadImage(url) => {
            tokio::spawn(async move {
                let (w, h) = crate::state::IMAGE_PX;
                let thumb = match app.image_thumbnail(&url, w, h).await {
                    Ok(thumb) => Some(thumb),
                    Err(error) => {
                        tracing::debug!(%error, "image unavailable");
                        None
                    }
                };
                let _ = tx.send(Msg::Image(url, thumb));
            });
        }
        Effect::OpenUrl(url) => {
            let _ = tx.send(open_url(&url));
        }
    }
}

async fn load(app: &Sentri, query: &FeedQuery) -> Msg {
    match app.feed(query).await {
        Ok(items) => Msg::Loaded(items),
        Err(e) => Msg::Failed(format!("Could not load the feed: {e}")),
    }
}

async fn overview(app: &Sentri, config_path: String) -> Msg {
    let stats = app.stats().await.unwrap_or_default();
    let insights = app.insights(30).await.unwrap_or_default();
    let news_query = FeedQuery {
        view: sentri_core::FeedView::News,
        sort: sentri_core::SortOrder::Newest,
        limit: Some(12),
        ..FeedQuery::default()
    };
    let news = app.feed(&news_query).await.unwrap_or_default();
    let runs = app.store().latest_runs().await.unwrap_or_default();
    Msg::Overview(Box::new(Overview {
        stats,
        insights,
        news,
        runs,
        sources: app.config().sources.clone(),
        profile: app.config().profile.clone(),
        config_path,
        database_path: app.config().database_path().display().to_string(),
    }))
}

fn describe(action: Action) -> String {
    match action {
        Action::Set(flag) => format!("Marked as {flag}"),
        Action::Clear(flag) => format!("Removed {flag}"),
        Action::Edited => "Saved changes".into(),
        Action::Scheduled => "Added to calendar".into(),
        Action::Viewed => String::new(),
    }
}

/// Opens a link with `xdg-open` (no shell involved). Over SSH there is no
/// local browser, so the URL is shown instead.
fn open_url(url: &str) -> Msg {
    let graphical =
        std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();
    let safe = url.starts_with("https://") || url.starts_with("http://");
    if !safe {
        return Msg::Failed("Refusing to open a non-http link".into());
    }
    if !graphical {
        return Msg::Info(format!("No local browser (SSH?). Link: {url}"));
    }
    match std::process::Command::new("xdg-open")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            // Reap the child so it does not linger as a zombie.
            std::thread::spawn(move || child.wait());
            Msg::Info(format!("Opened {url}"))
        }
        Err(e) => Msg::Failed(format!("Could not open the browser ({e}). Link: {url}")),
    }
}
