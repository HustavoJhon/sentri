//! Calendar use cases. Google Calendar sync is not connected yet (it needs
//! OAuth credentials, see docs/calendar.md); everything here is local.

use chrono::{DateTime, Utc};
use sentri_core::calendar::sync_policy;
use sentri_core::{CalendarEntry, FeedItem, OpportunityId, SyncPolicy};
use sentri_storage::NewEntry;

use crate::{AppError, Sentri};

/// What the calendar shows for a period.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CalendarData {
    /// Entries in SENTRI's calendar.
    pub entries: Vec<CalendarEntry>,
    /// Dated opportunities in the period not yet added (suggestions).
    pub suggestions: Vec<FeedItem>,
}

/// State of the Google Calendar integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleSync {
    /// No OAuth credentials configured: nothing is synchronized.
    NotConnected,
}

impl Sentri {
    pub async fn calendar(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<CalendarData, AppError> {
        let entries = self.store().entries_between(from, to).await?;
        let scheduled: std::collections::HashSet<i64> =
            entries.iter().filter_map(|e| e.opportunity_id).collect();
        let suggestions = self
            .store()
            .dated_between(from, to)
            .await?
            .into_iter()
            .filter(|i| !scheduled.contains(&i.id.0))
            .collect();
        Ok(CalendarData {
            entries,
            suggestions,
        })
    }

    /// Adds an opportunity's dates to SENTRI's calendar. This does not
    /// register or apply to anything.
    pub async fn schedule(&self, id: OpportunityId) -> Result<Vec<CalendarEntry>, AppError> {
        Ok(self.store().schedule_opportunity(id, Utc::now()).await?)
    }

    pub async fn create_entry(&self, new: &NewEntry) -> Result<CalendarEntry, AppError> {
        Ok(self.store().create_entry(new, Utc::now()).await?)
    }

    pub async fn update_entry(&self, id: i64, new: &NewEntry) -> Result<CalendarEntry, AppError> {
        Ok(self.store().update_entry(id, new, Utc::now()).await?)
    }

    pub async fn delete_entry(&self, id: i64) -> Result<(), AppError> {
        Ok(self.store().delete_entry(id).await?)
    }

    /// How an opportunity would sync once Google Calendar is connected.
    pub fn sync_policy(&self, item: &FeedItem) -> SyncPolicy {
        sync_policy(&item.opportunity, &self.config().profile)
    }

    pub const fn google_sync(&self) -> GoogleSync {
        GoogleSync::NotConnected
    }
}
