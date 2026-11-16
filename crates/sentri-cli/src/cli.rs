//! Command line definition.

use std::path::PathBuf;

use chrono::{DateTime, NaiveDate, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use sentri_core::{Category, FeedView, Flag, Modality, SortOrder, SourceKind};

#[derive(Debug, Parser)]
#[command(
    name = "sentri",
    version,
    about = "SENTRI — personal opportunity radar",
    long_about = "SENTRI — personal opportunity radar.\n\nRun without a command to open the TUI."
)]
pub struct Cli {
    /// Configuration file (default: `~/.config/sentri/config.toml` or `$SENTRI_CONFIG`).
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Add a link found while browsing.
    Add(AddArgs),
    /// Correct or complete an opportunity's data.
    Edit {
        id: i64,
        #[command(flatten)]
        fields: FieldArgs,
        /// Set any field: `--set author_handle=@ana` (see `sentri fields`).
        #[arg(long = "set", value_name = "FIELD=VALUE")]
        set: Vec<String>,
        /// Restore a field to its detected value (repeatable).
        #[arg(long = "reset", value_name = "FIELD")]
        reset: Vec<String>,
        /// Personal notes (empty string clears them).
        #[arg(long)]
        notes: Option<String>,
        /// Personal priority: low, medium or high.
        #[arg(long)]
        priority: Option<String>,
    },
    /// List editable fields.
    Fields,
    /// Upcoming calendar entries and dated opportunities.
    Calendar {
        #[arg(long, default_value_t = 30)]
        days: i64,
    },
    /// Add an opportunity's dates to SENTRI's calendar (does not enroll you).
    Schedule { id: i64 },
    /// Synchronize with Google Calendar.
    Sync,
    /// Check the Google Calendar setup (never sends anything to Google).
    CalendarDoctor,
    /// Re-apply the current news stance rules to stored news (dry run by
    /// default). Manual edits, likes, saves and notes are never changed.
    ReclassifyNews {
        /// Write the changes (a batch id is printed for undo).
        #[arg(long, conflicts_with = "undo")]
        apply: bool,
        /// Undo a previous batch.
        #[arg(long, value_name = "BATCH")]
        undo: Option<String>,
    },
    /// Search opportunities by keywords.
    Search {
        #[arg(required = true)]
        words: Vec<String>,
        #[command(flatten)]
        filters: FeedArgs,
    },
    /// Show the ranked feed.
    Feed {
        #[arg(long, value_enum, default_value_t = ViewArg::Discover)]
        view: ViewArg,
        #[command(flatten)]
        filters: FeedArgs,
    },
    /// Saved opportunities.
    Saved(FeedArgs),
    /// Liked opportunities.
    Liked(FeedArgs),
    /// Jobs and internships.
    Jobs(FeedArgs),
    /// Courses and training.
    Courses(FeedArgs),
    /// Events, conferences, workshops and meetups.
    Events(FeedArgs),
    /// Recent tech news.
    News(FeedArgs),
    /// Details of one opportunity and why it is recommended.
    Show { id: i64 },
    /// Toggle like.
    Like { id: i64 },
    /// Toggle saved.
    Save { id: i64 },
    /// Toggle dismissed.
    Dismiss { id: i64 },
    /// Set a status flag (applied, enrolled, completed, interesting…).
    Mark {
        id: i64,
        #[arg(value_enum)]
        flag: FlagArg,
    },
    /// Clear a status flag.
    Unmark {
        id: i64,
        #[arg(value_enum)]
        flag: FlagArg,
    },
    /// Recent actions.
    History {
        #[arg(long, default_value_t = 30)]
        limit: u32,
    },
    /// Database statistics and source status.
    Status,
    /// Fetch new opportunities from enabled sources now.
    Collect {
        /// Only this source (also works for disabled sources).
        #[arg(long)]
        source: Option<String>,
    },
    /// List configured sources and their last run.
    Sources,
    /// Collect continuously in the background (use with systemd).
    Daemon {
        /// Minutes between cycles (default: `daemon.interval_minutes`).
        #[arg(long, value_parser = clap::value_parser!(u64).range(15..))]
        interval: Option<u64>,
    },
    /// Manage the configuration file.
    #[command(subcommand)]
    Config(ConfigCommand),
    /// Write a consistent copy of the database (never overwrites).
    Backup { target: PathBuf },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Write the commented default configuration.
    Init,
    /// Print the configuration path in use.
    Path,
    /// Validate the configuration.
    Check,
}

#[derive(Debug, Args)]
pub struct AddArgs {
    pub url: String,
    #[command(flatten)]
    pub fields: FieldArgs,
}

/// Fields the user can set when adding or editing.
#[derive(Debug, Args, Default)]
pub struct FieldArgs {
    /// Title, if the page does not provide a good one.
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub summary: Option<String>,
    #[arg(long, value_parser = parse_category)]
    pub category: Option<Category>,
    #[arg(long, value_parser = parse_modality)]
    pub modality: Option<Modality>,
    #[arg(long)]
    pub location: Option<String>,
    /// Event or course start (YYYY-MM-DD or RFC 3339).
    #[arg(long, value_parser = parse_date)]
    pub starts: Option<DateTime<Utc>>,
    /// Application deadline (YYYY-MM-DD or RFC 3339).
    #[arg(long, value_parser = parse_date)]
    pub deadline: Option<DateTime<Utc>>,
    /// Extra tag (repeatable).
    #[arg(long = "tag")]
    pub tags: Vec<String>,
}

/// Feed filters; each flag is an independent toggle.
#[derive(Debug, Args, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct FeedArgs {
    #[arg(long, default_value_t = 25)]
    pub limit: u32,
    #[arg(long, value_enum, default_value_t = SortArg::Relevance)]
    pub sort: SortArg,
    #[arg(long, value_parser = parse_source)]
    pub source: Vec<SourceKind>,
    #[arg(long, value_parser = parse_category)]
    pub category: Vec<Category>,
    #[arg(long, value_parser = parse_modality)]
    pub modality: Vec<Modality>,
    #[arg(long)]
    pub location: Option<String>,
    /// Only free opportunities.
    #[arg(long, conflicts_with = "paid")]
    pub free: bool,
    /// Only paid opportunities.
    #[arg(long)]
    pub paid: bool,
    #[arg(long)]
    pub certificate: bool,
    #[arg(long)]
    pub prizes: bool,
    /// Only opportunities covering travel or accommodation.
    #[arg(long)]
    pub travel: bool,
    /// Only opportunities explicitly open to your country.
    #[arg(long)]
    pub eligible: bool,
    /// Hide opportunities whose deadline passed.
    #[arg(long)]
    pub upcoming: bool,
    #[arg(long)]
    pub min_score: Option<i32>,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum SortArg {
    #[default]
    Relevance,
    Newest,
    Deadline,
}

impl From<SortArg> for SortOrder {
    fn from(s: SortArg) -> Self {
        match s {
            SortArg::Relevance => Self::Relevance,
            SortArg::Newest => Self::Newest,
            SortArg::Deadline => Self::Deadline,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ViewArg {
    Discover,
    News,
    All,
    Jobs,
    Courses,
    Events,
    Hackathons,
    Scholarships,
    Volunteering,
    OpenSource,
    Travel,
    Liked,
    Saved,
    Interesting,
    Applied,
    Enrolled,
    Completed,
    Dismissed,
    History,
}

impl From<ViewArg> for FeedView {
    fn from(v: ViewArg) -> Self {
        match v {
            ViewArg::Discover => Self::Discover,
            ViewArg::News => Self::News,
            ViewArg::All => Self::All,
            ViewArg::Jobs => Self::Jobs,
            ViewArg::Courses => Self::Courses,
            ViewArg::Events => Self::Events,
            ViewArg::Hackathons => Self::Hackathons,
            ViewArg::Scholarships => Self::Scholarships,
            ViewArg::Volunteering => Self::Volunteering,
            ViewArg::OpenSource => Self::OpenSource,
            ViewArg::Travel => Self::Travel,
            ViewArg::Liked => Self::Liked,
            ViewArg::Saved => Self::Saved,
            ViewArg::Interesting => Self::Interesting,
            ViewArg::Applied => Self::Applied,
            ViewArg::Enrolled => Self::Enrolled,
            ViewArg::Completed => Self::Completed,
            ViewArg::Dismissed => Self::Dismissed,
            ViewArg::History => Self::History,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FlagArg {
    Liked,
    Saved,
    Dismissed,
    Interesting,
    Applied,
    Enrolled,
    Completed,
}

impl From<FlagArg> for Flag {
    fn from(f: FlagArg) -> Self {
        match f {
            FlagArg::Liked => Self::Liked,
            FlagArg::Saved => Self::Saved,
            FlagArg::Dismissed => Self::Dismissed,
            FlagArg::Interesting => Self::Interesting,
            FlagArg::Applied => Self::Applied,
            FlagArg::Enrolled => Self::Enrolled,
            FlagArg::Completed => Self::Completed,
        }
    }
}

fn parse_category(s: &str) -> Result<Category, String> {
    s.parse().map_err(|e| format!("{e}"))
}

fn parse_modality(s: &str) -> Result<Modality, String> {
    s.parse().map_err(|e| format!("{e}"))
}

fn parse_source(s: &str) -> Result<SourceKind, String> {
    s.parse().map_err(|e| format!("{e}"))
}

/// Accepts `YYYY-MM-DD` (midnight UTC) or a full RFC 3339 timestamp.
pub fn parse_date(s: &str) -> Result<DateTime<Utc>, String> {
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(date.and_hms_opt(0, 0, 0).unwrap_or_default().and_utc());
    }
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.to_utc())
        .map_err(|_| format!("invalid date {s:?}; use YYYY-MM-DD"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_add_with_overrides() {
        let cli = Cli::parse_from([
            "sentri",
            "add",
            "https://example.com",
            "--category",
            "hackathon",
            "--deadline",
            "2026-12-01",
            "--tag",
            "ai",
        ]);
        let Some(Command::Add(args)) = cli.command else {
            panic!("expected add");
        };
        assert_eq!(args.fields.category, Some(Category::Hackathon));
        assert_eq!(args.fields.tags, ["ai"]);
        assert_eq!(
            args.fields.deadline.unwrap().to_rfc3339(),
            "2026-12-01T00:00:00+00:00"
        );
    }

    #[test]
    fn rejects_bad_dates() {
        assert!(parse_date("01/12/2026").is_err());
    }
}
