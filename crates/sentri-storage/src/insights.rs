//! Aggregates for the dashboard charts and the activity heatmap.
//!
//! Days are computed in the caller's local time through `utc_offset_secs`,
//! because "today" for a user in Cusco is not the UTC day. Every number comes
//! from persisted rows; nothing is estimated.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use sentri_core::{Category, Modality, SourceKind};
use sqlx::Row;

use crate::time::to_db;
use crate::{StorageError, Store};

/// Counts per day.
pub type Daily = BTreeMap<NaiveDate, i64>;

/// Everything the dashboard draws.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Insights {
    pub discovered_per_day: Daily,
    pub likes_per_day: Daily,
    pub saves_per_day: Daily,
    pub by_category: Vec<(Category, i64)>,
    pub by_source: Vec<(SourceKind, i64)>,
    pub by_modality: Vec<(Modality, i64)>,
    pub news_total: i64,
    pub news_new: i64,
    pub upcoming_events: i64,
    pub pending_courses: i64,
    pub interesting_jobs: i64,
    pub calendar_entries: i64,
    pub applied: i64,
    pub enrolled: i64,
    pub completed: i64,
}

/// One day of the activity heatmap.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DayActivity {
    pub discovered: i64,
    pub added_manually: i64,
    /// Distinct (opportunity, action) pairs per action name.
    pub actions: BTreeMap<String, i64>,
}

impl DayActivity {
    /// Total used for the heatmap intensity. Views are excluded: opening
    /// details is browsing, not activity worth a streak.
    pub fn total(&self) -> i64 {
        self.added_manually
            + self
                .actions
                .iter()
                .filter(|(action, _)| action.as_str() != "viewed")
                .map(|(_, n)| n)
                .sum::<i64>()
    }
}

/// An action performed on a given day, for the day summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayEntry {
    pub opportunity_id: i64,
    pub title: String,
    pub action: String,
}

fn day_expr(column: &str) -> String {
    format!("date({column} + ?, 'unixepoch')")
}

fn parse_day(s: &str) -> Result<NaiveDate, StorageError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| StorageError::Corrupt {
        column: "day",
        value: s.to_owned(),
    })
}

impl Store {
    async fn daily(
        &self,
        sql: &str,
        offset: i64,
        since: DateTime<Utc>,
    ) -> Result<Daily, StorageError> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
            .bind(offset)
            .bind(to_db(since))
            .fetch_all(&self.pool)
            .await?;
        rows.iter()
            .map(|row| Ok((parse_day(row.try_get("day")?)?, row.try_get("n")?)))
            .collect()
    }

    async fn scalar(&self, sql: &'static str, bind: Option<i64>) -> Result<i64, StorageError> {
        let mut query = sqlx::query_scalar(sql);
        if let Some(value) = bind {
            query = query.bind(value);
        }
        Ok(query.fetch_one(&self.pool).await?)
    }

    /// Dashboard aggregates over the last `days` days.
    pub async fn insights(
        &self,
        now: DateTime<Utc>,
        days: i64,
        utc_offset_secs: i64,
    ) -> Result<Insights, StorageError> {
        let since = now - chrono::Duration::days(days);
        let discovered = format!(
            "SELECT {} AS day, COUNT(*) AS n FROM opportunities
             WHERE discovered_at >= ? GROUP BY day",
            day_expr("discovered_at")
        );
        let action = |name: &str| {
            format!(
                "SELECT day, COUNT(*) AS n FROM (
                    SELECT DISTINCT opportunity_id, {} AS day FROM interaction_events
                    WHERE at >= ? AND action = 'set:{name}')
                 GROUP BY day",
                day_expr("at")
            )
        };
        let mut insights = Insights {
            discovered_per_day: self.daily(&discovered, utc_offset_secs, since).await?,
            likes_per_day: self.daily(&action("liked"), utc_offset_secs, since).await?,
            saves_per_day: self.daily(&action("saved"), utc_offset_secs, since).await?,
            ..Insights::default()
        };

        let grouped = |column: &str| {
            format!(
                "SELECT o.{column} AS k, COUNT(*) AS n FROM opportunities o
                 LEFT JOIN interactions i ON i.opportunity_id = o.id
                 WHERE COALESCE(i.dismissed, 0) = 0 AND o.content_kind = 'opportunity'
                 GROUP BY o.{column} ORDER BY n DESC"
            )
        };
        for row in sqlx::query(sqlx::AssertSqlSafe(grouped("category")))
            .fetch_all(&self.pool)
            .await?
        {
            if let Ok(c) = row.try_get::<&str, _>("k")?.parse() {
                insights.by_category.push((c, row.try_get("n")?));
            }
        }
        for row in sqlx::query(sqlx::AssertSqlSafe(grouped("modality")))
            .fetch_all(&self.pool)
            .await?
        {
            if let Ok(m) = row.try_get::<&str, _>("k")?.parse() {
                insights.by_modality.push((m, row.try_get("n")?));
            }
        }
        for row in sqlx::query(
            "SELECT source_kind AS k, COUNT(*) AS n FROM opportunities
             GROUP BY source_kind ORDER BY n DESC",
        )
        .fetch_all(&self.pool)
        .await?
        {
            if let Ok(s) = row.try_get::<&str, _>("k")?.parse() {
                insights.by_source.push((s, row.try_get("n")?));
            }
        }

        self.counters(&mut insights, now).await?;
        Ok(insights)
    }

    async fn counters(
        &self,
        insights: &mut Insights,
        now: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let day_ago = to_db(now - chrono::Duration::hours(24));
        let now_db = to_db(now);
        insights.news_total = self
            .scalar(
                "SELECT COUNT(*) FROM opportunities WHERE content_kind = 'news'",
                None,
            )
            .await?;
        insights.news_new = self
            .scalar(
                "SELECT COUNT(*) FROM opportunities WHERE content_kind = 'news' AND discovered_at >= ?",
                Some(day_ago),
            )
            .await?;
        insights.upcoming_events = self
            .scalar(
                "SELECT COUNT(*) FROM opportunities WHERE starts_at >= ?
                 AND category IN ('event','conference','workshop','meetup','hackathon')",
                Some(now_db),
            )
            .await?;
        insights.pending_courses = self
            .scalar(
                "SELECT COUNT(*) FROM opportunities o JOIN interactions i ON i.opportunity_id = o.id
                 WHERE o.category IN ('course','training') AND (i.saved = 1 OR i.enrolled = 1)
                   AND i.completed = 0",
                None,
            )
            .await?;
        insights.interesting_jobs = self
            .scalar(
                "SELECT COUNT(*) FROM opportunities o JOIN interactions i ON i.opportunity_id = o.id
                 WHERE o.category IN ('job','internship')
                   AND (i.saved = 1 OR i.liked = 1 OR i.interesting = 1) AND i.dismissed = 0",
                None,
            )
            .await?;
        insights.calendar_entries = self
            .scalar(
                "SELECT COUNT(*) FROM calendar_entries WHERE starts_at >= ?",
                Some(now_db),
            )
            .await?;
        insights.applied = self
            .scalar("SELECT COUNT(*) FROM interactions WHERE applied = 1", None)
            .await?;
        insights.enrolled = self
            .scalar("SELECT COUNT(*) FROM interactions WHERE enrolled = 1", None)
            .await?;
        insights.completed = self
            .scalar(
                "SELECT COUNT(*) FROM interactions WHERE completed = 1",
                None,
            )
            .await?;
        Ok(())
    }

    /// Activity per local day since `since`, deduplicated per opportunity,
    /// action and day.
    pub async fn activity(
        &self,
        since: DateTime<Utc>,
        utc_offset_secs: i64,
    ) -> Result<BTreeMap<NaiveDate, DayActivity>, StorageError> {
        let mut days: BTreeMap<NaiveDate, DayActivity> = BTreeMap::new();
        let discovered = format!(
            "SELECT {} AS day, source_name = 'manual' AS manual, COUNT(*) AS n
             FROM opportunities WHERE discovered_at >= ? GROUP BY day, manual",
            day_expr("discovered_at")
        );
        for row in sqlx::query(sqlx::AssertSqlSafe(discovered))
            .bind(utc_offset_secs)
            .bind(to_db(since))
            .fetch_all(&self.pool)
            .await?
        {
            let entry = days.entry(parse_day(row.try_get("day")?)?).or_default();
            let n: i64 = row.try_get("n")?;
            if row.try_get::<bool, _>("manual")? {
                entry.added_manually += n;
            }
            entry.discovered += n;
        }
        let actions = format!(
            "SELECT day, action, COUNT(*) AS n FROM (
                SELECT DISTINCT opportunity_id, action, {} AS day FROM interaction_events
                WHERE at >= ? AND action NOT LIKE 'clear:%')
             GROUP BY day, action",
            day_expr("at")
        );
        for row in sqlx::query(sqlx::AssertSqlSafe(actions))
            .bind(utc_offset_secs)
            .bind(to_db(since))
            .fetch_all(&self.pool)
            .await?
        {
            let action: String = row.try_get("action")?;
            let action = action.trim_start_matches("set:").to_owned();
            *days
                .entry(parse_day(row.try_get("day")?)?)
                .or_default()
                .actions
                .entry(action)
                .or_default() += row.try_get::<i64, _>("n")?;
        }
        Ok(days)
    }

    /// What happened on one local day (for the heatmap detail panel).
    pub async fn day_entries(
        &self,
        day: NaiveDate,
        utc_offset_secs: i64,
    ) -> Result<Vec<DayEntry>, StorageError> {
        let sql = format!(
            "SELECT DISTINCT e.opportunity_id, o.title, e.action FROM interaction_events e
             JOIN opportunities o ON o.id = e.opportunity_id
             WHERE {} = ? AND e.action NOT LIKE 'clear:%'
             UNION
             SELECT id, title, CASE WHEN source_name = 'manual' THEN 'added' ELSE 'discovered' END
             FROM opportunities WHERE {} = ?
             ORDER BY 3, 2",
            day_expr("e.at"),
            day_expr("discovered_at")
        );
        let day = day.format("%Y-%m-%d").to_string();
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(utc_offset_secs)
            .bind(&day)
            .bind(utc_offset_secs)
            .bind(&day)
            .fetch_all(&self.pool)
            .await?;
        rows.iter()
            .map(|row| {
                let action: String = row.try_get(2)?;
                Ok(DayEntry {
                    opportunity_id: row.try_get(0)?,
                    title: row.try_get(1)?,
                    action: action.trim_start_matches("set:").to_owned(),
                })
            })
            .collect()
    }
}
