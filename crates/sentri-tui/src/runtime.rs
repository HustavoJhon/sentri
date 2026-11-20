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

use crate::state::{Effect, Msg, Overview, ProfileState, State};
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
    // Ask about graphics before the reader thread starts consuming stdin.
    let graphics = crate::graphics::Graphics::detect(ui.images);
    if crate::graphics::wants_probe(ui.images) {
        drain_terminal_replies();
    }
    let (event_tx, mut events) = mpsc::unbounded_channel();
    spawn_terminal_reader(event_tx);
    let (msg_tx, mut msgs) = mpsc::unbounded_channel();

    let mut state = State {
        loading: true,
        density: crate::state::Density::from_name(&ui.density).unwrap_or_default(),
        ui: crate::state::UiPrefs::from_config(ui, palette == Palette::INDEXED),
        cards: crate::state::presentation::CardPrefs::from_config(&app.config().cards),
        dashboard: crate::state::presentation::DashboardPrefs::from_config(&app.config().dashboard),
        keymap: crate::state::keymap(&app.config().keys),
        graphics,
        calendar: crate::state::calendar::CalendarState {
            view: crate::state::calendar::CalView::from_name(&app.config().calendar.default_view),
            sunday_first: app.config().calendar.week_start == "sunday",
            ..crate::state::calendar::CalendarState::default()
        },
        ..State::default()
    };
    execute(&app, &state, Effect::Load, &config_path, &msg_tx);
    execute(&app, &state, Effect::LoadProfile, &config_path, &msg_tx);

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

/// Some terminals (and tmux) answer the graphics query with more than the
/// probe reads; the rest would be parsed together with the first key press
/// and swallow it. Whatever arrives right after the probe is a reply, not
/// typing, so it is discarded.
fn drain_terminal_replies() {
    let deadline = std::time::Instant::now() + Duration::from_millis(150);
    while std::time::Instant::now() < deadline {
        match event::poll(Duration::from_millis(30)) {
            Ok(true) => {
                let _ = event::read();
            }
            Ok(false) | Err(_) => break,
        }
    }
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

/// Sources fetched at the same time. Most live on different hosts; the few
/// that share one (Google News, Get on Board) stay well under rate limits.
const COLLECT_PARALLELISM: usize = 4;

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
        Effect::RefreshOverview => {
            let config_path = config_path.to_owned();
            tokio::spawn(async move {
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
        Effect::Collect => execute_collect(app, tx),
        Effect::LoadProfile => load_profile(app, tx),
        Effect::CalendarDoctor => {
            let d = sentri_app::google::diagnose(&app.config().calendar.credentials_path());
            let failed: Vec<String> = d
                .checks
                .iter()
                .filter(|c| !c.ok)
                .map(|c| format!("{}: {}", c.label, c.detail))
                .collect();
            let _ = tx.send(Msg::Info(format!(
                "Google Calendar not connected · {} · run `sentri calendar-doctor` for steps",
                failed.join(" · ")
            )));
        }
        source @ (Effect::SaveSource { .. }
        | Effect::DeleteSource(_)
        | Effect::ToggleSource(..)
        | Effect::RefreshSource(_)) => execute_source(app, config_path, source, tx),
        settings @ (Effect::SaveSettings(_) | Effect::RestoreDefaults) => {
            let msg = execute_settings(config_path, settings);
            if let Msg::SettingsSaved(saved) = &msg {
                // Profile, scoring and sources apply immediately.
                app.reload_config(saved.config.clone());
                load_profile(app.clone(), tx.clone());
            }
            let _ = tx.send(msg);
        }
        history @ (Effect::LoadActivity | Effect::LoadDay(_)) => execute_history(app, &history, tx),
        calendar @ (Effect::Schedule(_)
        | Effect::LoadCalendar(..)
        | Effect::CreateEntry(_)
        | Effect::UpdateEntry(..)
        | Effect::DeleteEntry(_)) => execute_calendar(app, calendar, tx),
        detail @ (Effect::LoadDetail(_)
        | Effect::LoadImage(_)
        | Effect::OpenEditor(_)
        | Effect::SaveEdit { .. }) => execute_detail(app, detail, tx),
        Effect::OpenUrl(url) => {
            let _ = tx.send(open_url(&url));
        }
    }
}

/// Collects every enabled source in parallel, reporting each as it ends.
fn execute_collect(app: Sentri, tx: UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let enabled = app.config().sources.iter().filter(|s| s.enabled).count();
        let _ = tx.send(Msg::CollectStarted(enabled));
        let mut new = 0;
        let mut failed = Vec::new();
        let progress = tx.clone();
        let total = app
            .collect_each(COLLECT_PARALLELISM, |report| {
                match &report.result {
                    Ok((_, n)) => new += n,
                    Err(_) => failed.push(report.source.clone()),
                }
                let _ = progress.send(Msg::SourceCollected(report));
            })
            .await;
        let mut text = format!("Collected from {total} sources: {new} new");
        if !failed.is_empty() {
            failed.sort();
            text.push_str(" · failed: ");
            text.push_str(&failed.join(", "));
        }
        let _ = tx.send(Msg::Collected(text));
    });
}

/// Source management: test, save (with backup), toggle, delete, refresh.
fn execute_source(app: Sentri, config_path: &str, effect: Effect, tx: UnboundedSender<Msg>) {
    let path = std::path::PathBuf::from(config_path);
    tokio::spawn(async move {
        use sentri_app::settings;
        let saved = match effect {
            Effect::SaveSource { original, draft } => {
                let test = match sentri_app::draft_config(&draft) {
                    Ok(config) => app.test_source(&config).await,
                    Err(e) => Err(e),
                };
                let result = match &original {
                    Some(name) => settings::update_source(&path, name, &draft),
                    None => settings::add_source(&path, &draft),
                };
                result.map(|saved| {
                    let note = match test {
                        Ok(n) => format!("Saved {} · test found {n} item(s)", draft.name),
                        Err(e) => format!("Saved {} · warning: test failed: {e}", draft.name),
                    };
                    (saved, note)
                })
            }
            Effect::DeleteSource(name) => settings::remove_source(&path, &name).map(|saved| {
                let store = app.store().clone();
                let forget = name.clone();
                tokio::spawn(async move { store.forget_source(&forget).await });
                (
                    saved,
                    format!("Deleted {name} (its items stay in your feed)"),
                )
            }),
            Effect::ToggleSource(name, enabled) => {
                let key = format!("sources.{name}.enabled");
                settings::save(&path, &[(key, enabled.to_string())]).map(|saved| {
                    let verb = if enabled { "Enabled" } else { "Disabled" };
                    (saved, format!("{verb} {name}"))
                })
            }
            Effect::RefreshSource(name) => {
                let reports = app.collect(Some(&name)).await;
                let text = reports.first().map_or_else(
                    || format!("{name}: not found"),
                    |r| match (&r.result, &r.note) {
                        (Ok(_), Some(note)) => format!("{name}: {note}"),
                        (Ok((seen, new)), None) => format!("{name}: {seen} seen, {new} new"),
                        (Err(e), _) => format!("{name} failed: {e}"),
                    },
                );
                let _ = tx.send(Msg::Collected(text));
                return;
            }
            _ => return,
        };
        let msg = match saved {
            Ok((saved, note)) => {
                app.reload_config(saved.config.clone());
                Msg::SourcesSaved(Box::new(saved), note)
            }
            Err(e) => Msg::Failed(format!("Sources not changed: {e}")),
        };
        let _ = tx.send(msg);
    });
}

/// Writes settings to the configuration file (with a backup).
fn execute_settings(config_path: &str, effect: Effect) -> Msg {
    let path = std::path::PathBuf::from(config_path);
    let result = match effect {
        Effect::SaveSettings(changes) => sentri_app::settings::save(&path, &changes),
        _ => sentri_app::settings::restore_defaults(&path),
    };
    match result {
        Ok(saved) => Msg::SettingsSaved(Box::new(saved)),
        Err(e) => Msg::Failed(format!("Settings not saved: {e}")),
    }
}

/// Loads the GitHub identity shown in the sidebar.
fn load_profile(app: Sentri, tx: UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let profile = match app.github_profile().await {
            Ok(None) => ProfileState::NotConfigured,
            Ok(Some(profile)) => {
                let avatar = app.github_avatar(&profile).await.ok();
                ProfileState::Ready(Box::new(profile), avatar)
            }
            Err(e) => ProfileState::Unavailable(e.to_string()),
        };
        let _ = tx.send(Msg::Profile(profile));
    });
}

/// History section loads.
fn execute_history(app: Sentri, effect: &Effect, tx: UnboundedSender<Msg>) {
    match effect {
        Effect::LoadActivity => {
            tokio::spawn(async move {
                let msg = match app.activity(crate::state::history::DAYS).await {
                    Ok(days) => Msg::Activity(days),
                    Err(e) => Msg::Failed(format!("Could not load activity: {e}")),
                };
                let _ = tx.send(msg);
            });
        }
        &Effect::LoadDay(day) => {
            tokio::spawn(async move {
                let msg = match app.day_entries(day).await {
                    Ok(entries) => Msg::DayEntries(day, entries),
                    Err(e) => Msg::Failed(format!("Could not load {day}: {e}")),
                };
                let _ = tx.send(msg);
            });
        }
        _ => {}
    }
}

/// Calendar effects. Nothing here talks to Google Calendar.
fn execute_calendar(app: Sentri, effect: Effect, tx: UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let msg = match effect {
            Effect::LoadCalendar(from, to) => match app.calendar(from, to).await {
                Ok(data) => Msg::Calendar(Box::new(data)),
                Err(e) => Msg::Failed(format!("Could not load the calendar: {e}")),
            },
            Effect::Schedule(id) => match app.schedule(id).await {
                Ok(entries) => Msg::CalendarChanged(format!(
                    "Added {} date(s) to SENTRI's calendar (this does not register you)",
                    entries.len()
                )),
                Err(e) => Msg::Failed(format!("Could not add to calendar: {e}")),
            },
            Effect::CreateEntry(entry) => match app.create_entry(&entry).await {
                Ok(e) => Msg::CalendarChanged(format!("Created \"{}\"", e.title)),
                Err(e) => Msg::Failed(format!("Could not create the entry: {e}")),
            },
            Effect::UpdateEntry(id, entry) => match app.update_entry(id, &entry).await {
                Ok(e) => Msg::CalendarChanged(format!("Updated \"{}\"", e.title)),
                Err(e) => Msg::Failed(format!("Could not update the entry: {e}")),
            },
            Effect::DeleteEntry(id) => match app.delete_entry(id).await {
                Ok(()) => Msg::CalendarChanged("Entry deleted".into()),
                Err(e) => Msg::Failed(format!("Could not delete the entry: {e}")),
            },
            _ => return,
        };
        let _ = tx.send(msg);
    });
}

/// Effects behind Details and the editor.
fn execute_detail(app: Sentri, effect: Effect, tx: UnboundedSender<Msg>) {
    match effect {
        Effect::LoadDetail(id) => {
            tokio::spawn(async move {
                let msg = match (
                    app.edit_state(id).await,
                    app.history_for(id, 20).await,
                    app.sightings(id).await,
                ) {
                    (Ok(edit), Ok(history), Ok(sightings)) => {
                        Msg::Detail(Box::new(crate::state::DetailData {
                            id,
                            edit,
                            history,
                            sightings,
                        }))
                    }
                    (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                        Msg::Failed(format!("Could not load #{id}: {e}"))
                    }
                };
                let _ = tx.send(msg);
            });
        }
        Effect::LoadImage(url) => {
            tokio::spawn(async move {
                let thumb = match app.image(&url).await {
                    Ok(thumb) => Some(thumb),
                    Err(error) => {
                        tracing::debug!(%error, "image unavailable");
                        None
                    }
                };
                let _ = tx.send(Msg::Image(url, thumb));
            });
        }
        Effect::OpenEditor(id) => {
            tokio::spawn(async move {
                let msg = match app.edit_state(id).await {
                    Ok(edit) => Msg::EditorReady(id, Box::new(edit)),
                    Err(e) => Msg::Failed(format!("Cannot edit #{id}: {e}")),
                };
                let _ = tx.send(msg);
            });
        }
        Effect::SaveEdit {
            id,
            values,
            reset,
            annotations,
        } => {
            tokio::spawn(async move {
                let msg = match app.save_edit(id, &values, &reset, Some(&annotations)).await {
                    Ok(()) => Msg::Saved(id),
                    Err(e) => Msg::Failed(format!("Could not save #{id}: {e}")),
                };
                let _ = tx.send(msg);
            });
        }
        _ => {}
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
    let counts = app
        .view_counts(&crate::state::Section::counted_views())
        .await
        .unwrap_or_default();
    let source_health = app.store().source_states().await.unwrap_or_default();
    Msg::Overview(Box::new(Overview {
        config: (*app.config()).clone(),
        states: source_health,
        stats,
        insights,
        news,
        counts,
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
