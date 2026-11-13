//! `sentri` binary entry point.

mod cli;
mod output;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use sentri_app::{AddOutcome, Config, LinkOverrides, Sentri, paths};
use sentri_core::{Action, FeedQuery, FeedView, Field, FieldKind, OpportunityId, PriceFilter};
use tracing_subscriber::EnvFilter;

use crate::cli::{AddArgs, Cli, Command, ConfigCommand, FeedArgs, FieldArgs};

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();
    let cli = Cli::parse();
    let config_path = cli.config.clone().unwrap_or_else(paths::config_file);
    match cli.command {
        Some(Command::Config(cmd)) => config_command(&cmd, &config_path),
        Some(command) => {
            let app = open(&config_path).await?;
            run(&app, command).await
        }
        None => {
            let app = open(&config_path).await?;
            sentri_tui::run(app, config_path.display().to_string())
                .await
                .context("TUI failed")
        }
    }
}

/// Logs go to stderr; `SENTRI_LOG=debug` raises verbosity.
fn init_logging() {
    let filter = EnvFilter::try_from_env("SENTRI_LOG").unwrap_or_else(|_| EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

async fn open(config_path: &std::path::Path) -> Result<Sentri> {
    let config = Config::load(config_path)?;
    Sentri::open(config).await.context("cannot open SENTRI")
}

fn config_command(cmd: &ConfigCommand, path: &std::path::Path) -> Result<()> {
    match cmd {
        ConfigCommand::Init => {
            Config::write_default(path)?;
            println!("Wrote {}", path.display());
        }
        ConfigCommand::Path => println!("{}", path.display()),
        ConfigCommand::Check => {
            let config = Config::load(path)?;
            println!(
                "OK: {} interests, {} sources ({} enabled). Database: {}",
                config.profile.interests.len(),
                config.sources.len(),
                config.sources.iter().filter(|s| s.enabled).count(),
                config.database_path().display()
            );
        }
    }
    Ok(())
}

async fn run(app: &Sentri, command: Command) -> Result<()> {
    match command {
        Command::Add(args) => add(app, args).await?,
        Command::Edit {
            id,
            fields,
            set,
            reset,
            notes,
            priority,
        } => {
            edit(
                app,
                OpportunityId(id),
                fields,
                &set,
                &reset,
                notes,
                priority,
            )
            .await?;
        }
        Command::Fields => {
            for field in Field::ALL {
                let kind = match field.kind() {
                    FieldKind::Choice(options) => options.join("|"),
                    other => format!("{other:?}").to_lowercase(),
                };
                println!("{:<16} {:<16} {kind}", field.as_str(), field.label());
            }
        }
        Command::Search { words, filters } => {
            let mut query = feed_query(FeedView::All, &filters);
            query.text = Some(words.join(" "));
            print!("{}", output::feed(&app.feed(&query).await?));
        }
        Command::Feed { view, filters } => list(app, view.into(), &filters).await?,
        Command::Saved(f) => list(app, FeedView::Saved, &f).await?,
        Command::Liked(f) => list(app, FeedView::Liked, &f).await?,
        Command::Jobs(f) => list(app, FeedView::Jobs, &f).await?,
        Command::Courses(f) => list(app, FeedView::Courses, &f).await?,
        Command::Events(f) => list(app, FeedView::Events, &f).await?,
        Command::News(f) => list(app, FeedView::News, &f).await?,
        Command::Show { id } => {
            let id = OpportunityId(id);
            app.act(id, Action::Viewed).await?;
            print!("{}", output::details(&app.explain(id).await?));
        }
        Command::Like { id } => toggle(app, id, sentri_core::Flag::Liked).await?,
        Command::Save { id } => toggle(app, id, sentri_core::Flag::Saved).await?,
        Command::Dismiss { id } => toggle(app, id, sentri_core::Flag::Dismissed).await?,
        Command::Mark { id, flag } => {
            app.act(OpportunityId(id), Action::Set(flag.into())).await?;
            println!("#{id}: {} set", sentri_core::Flag::from(flag));
        }
        Command::Unmark { id, flag } => {
            app.act(OpportunityId(id), Action::Clear(flag.into()))
                .await?;
            println!("#{id}: {} cleared", sentri_core::Flag::from(flag));
        }
        Command::History { limit } => {
            for entry in app.history(limit).await? {
                println!(
                    "{}  {:<16}  #{} {}",
                    entry.at.format("%Y-%m-%d %H:%M"),
                    entry.action.as_db(),
                    entry.opportunity_id,
                    entry.title
                );
            }
        }
        Command::Status => status(app).await?,
        Command::Collect { source } => collect(app, source.as_deref()).await?,
        Command::Sources => sources(app).await?,
        Command::Daemon { interval } => {
            let minutes = interval.unwrap_or(app.config().daemon.interval_minutes);
            app.run_daemon(Duration::from_secs(minutes * 60), shutdown_signal())
                .await?;
        }
        Command::Backup { target } => backup(app, target).await?,
        Command::Config(_) => unreachable!("handled before opening the database"),
    }
    Ok(())
}

fn overrides(fields: FieldArgs) -> LinkOverrides {
    LinkOverrides {
        title: fields.title,
        summary: fields.summary,
        category: fields.category,
        modality: fields.modality,
        location: fields.location,
        starts_at: fields.starts,
        deadline: fields.deadline,
        tags: fields.tags,
    }
}

async fn add(app: &Sentri, args: AddArgs) -> Result<()> {
    match app.add_link(&args.url, &overrides(args.fields)).await? {
        AddOutcome::Added { item, warnings } => {
            println!("Added #{}: {}", item.id, item.opportunity.title);
            for warning in warnings {
                println!("  ! {warning}");
            }
            println!("  Run `sentri show {}` to review it.", item.id);
        }
        AddOutcome::Duplicate(item) => {
            println!(
                "Already in SENTRI as #{}: {}",
                item.id, item.opportunity.title
            );
        }
    }
    Ok(())
}

async fn edit(
    app: &Sentri,
    id: OpportunityId,
    fields: FieldArgs,
    set: &[String],
    reset: &[String],
    notes: Option<String>,
    priority: Option<String>,
) -> Result<()> {
    let current = app.store().get(id).await?.opportunity;
    let mut values = overrides(fields).values(&current);
    for pair in set {
        let (field, value) = pair
            .split_once('=')
            .with_context(|| format!("expected FIELD=VALUE, got {pair:?}"))?;
        values.push((field.trim().parse::<Field>()?, value.to_owned()));
    }
    let reset: Vec<Field> = reset
        .iter()
        .map(|f| f.trim().parse::<Field>())
        .collect::<Result<_, _>>()?;
    let annotations = if notes.is_some() || priority.is_some() {
        let mut a = app.edit_state(id).await?.annotations;
        if let Some(notes) = notes {
            a.notes = Some(notes).filter(|n| !n.trim().is_empty());
        }
        if let Some(p) = priority {
            a.priority = Some(
                sentri_core::Priority::from_db(&p)
                    .with_context(|| format!("priority must be low, medium or high, not {p:?}"))?,
            );
        }
        Some(a)
    } else {
        None
    };
    app.save_edit(id, &values, &reset, annotations.as_ref())
        .await?;
    let state = app.edit_state(id).await?;
    println!("Updated #{id}: {}", state.effective.title);
    for o in &state.overrides {
        println!(
            "  ✎ {:<14} {}  (detected: {})",
            o.field.label(),
            o.value,
            Some(o.field.get(&state.detected))
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "—".into())
        );
    }
    Ok(())
}

fn feed_query(view: FeedView, f: &FeedArgs) -> FeedQuery {
    FeedQuery {
        view,
        sources: f.source.clone(),
        categories: f.category.clone(),
        modalities: f.modality.clone(),
        location: f.location.clone(),
        price: match (f.free, f.paid) {
            (true, _) => Some(PriceFilter::Free),
            (false, true) => Some(PriceFilter::Paid),
            (false, false) => None,
        },
        with_certificate: f.certificate,
        with_prizes: f.prizes,
        with_travel: f.travel,
        open_to_country: f.eligible,
        only_upcoming: f.upcoming,
        min_score: f.min_score,
        sort: f.sort.into(),
        limit: Some(f.limit),
        ..FeedQuery::default()
    }
}

async fn list(app: &Sentri, view: FeedView, filters: &FeedArgs) -> Result<()> {
    let items = app.feed(&feed_query(view, filters)).await?;
    print!("{}", output::feed(&items));
    Ok(())
}

async fn toggle(app: &Sentri, id: i64, flag: sentri_core::Flag) -> Result<()> {
    let state = app.toggle(OpportunityId(id), flag).await?;
    let verb = if state.get(flag) { "set" } else { "cleared" };
    println!("#{id}: {flag} {verb}");
    Ok(())
}

async fn status(app: &Sentri) -> Result<()> {
    let s = app.stats().await?;
    println!("Database   {}", app.config().database_path().display());
    println!(
        "Total {}  · new 24h {}  · liked {}  · saved {}  · applied {}  · upcoming deadlines {}",
        s.total, s.new_since, s.liked, s.saved, s.applied, s.upcoming_deadlines
    );
    let runs = app.store().latest_runs().await?;
    if runs.is_empty() {
        println!("No collection runs yet.");
    }
    for run in runs {
        println!(
            "  {:<28} {:?}  seen {} new {}  {}{}",
            run.source_name,
            run.status,
            run.items_seen,
            run.items_new,
            run.started_at.format("%Y-%m-%d %H:%M"),
            run.error
                .map(|e| format!("  error: {e}"))
                .unwrap_or_default()
        );
    }
    Ok(())
}

async fn collect(app: &Sentri, only: Option<&str>) -> Result<()> {
    if let Some(name) = only
        && app.config().source(name).is_none()
    {
        anyhow::bail!("no source named {name:?}; see `sentri sources`");
    }
    let reports = app.collect(only).await;
    if reports.is_empty() {
        println!(
            "No enabled sources. Enable some in the [[sources]] section of the configuration."
        );
    }
    for report in reports {
        match report.result {
            Ok((seen, new)) => println!("  ✓ {:<28} {seen} seen, {new} new", report.source),
            Err(error) => println!("  ✗ {:<28} {error}", report.source),
        }
    }
    Ok(())
}

async fn sources(app: &Sentri) -> Result<()> {
    let runs = app.store().latest_runs().await?;
    for source in &app.config().sources {
        let run = runs.iter().find(|r| r.source_name == source.name);
        println!(
            "{} [{:<3}] {:<28} {}",
            if source.enabled { "●" } else { "○" },
            source.kind.badge(),
            source.name,
            run.map_or_else(
                || "never run".to_owned(),
                |r| format!(
                    "{:?} {} · seen {} new {}{}",
                    r.status,
                    r.started_at.format("%Y-%m-%d %H:%M"),
                    r.items_seen,
                    r.items_new,
                    r.error
                        .as_deref()
                        .map(|e| format!(" · {e}"))
                        .unwrap_or_default()
                )
            )
        );
    }
    Ok(())
}

/// Resolves on Ctrl-C or SIGTERM (what systemd sends on stop).
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn backup(app: &Sentri, target: PathBuf) -> Result<()> {
    app.store().backup(&target).await?;
    println!("Backup written to {}", target.display());
    Ok(())
}
