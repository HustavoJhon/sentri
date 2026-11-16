//! Date conventions.
//!
//! A date without a time of day (`2026-11-21` in a form, a JSON-LD
//! `startDate` without time, an all-day entry) is stored as midnight UTC of
//! that date and must be displayed as that calendar date, never converted to
//! local time (which would move it to the previous evening west of UTC).

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};

/// Whether `d` represents a whole day rather than an instant.
pub fn is_all_day(d: DateTime<Utc>) -> bool {
    d.time() == NaiveTime::MIN
}

/// The calendar date a timestamp belongs to for a viewer whose local time
/// is `to_local(d)`. All-day values keep their own date.
pub fn calendar_date(
    d: DateTime<Utc>,
    local_date: impl FnOnce(DateTime<Utc>) -> NaiveDate,
) -> NaiveDate {
    if is_all_day(d) {
        d.date_naive()
    } else {
        local_date(d)
    }
}

/// The instant a date stops being upcoming: all-day values last until the
/// end of their day.
pub fn effective_end(d: DateTime<Utc>) -> DateTime<Utc> {
    if is_all_day(d) {
        d + chrono::Duration::days(1)
    } else {
        d
    }
}

/// Midnight UTC of a date: the stored form of an all-day value.
pub fn all_day(date: NaiveDate) -> DateTime<Utc> {
    date.and_time(NaiveTime::MIN).and_utc()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn all_day_values_keep_their_date() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 21).unwrap();
        let d = all_day(date);
        assert!(is_all_day(d));
        // A viewer at UTC-5 would see 2026-11-20 19:00 if converted.
        let west = |x: DateTime<Utc>| (x - Duration::hours(5)).date_naive();
        assert_eq!(calendar_date(d, west), date);
        let instant = d + Duration::hours(3);
        assert_eq!(
            calendar_date(instant, west),
            NaiveDate::from_ymd_opt(2026, 11, 20).unwrap()
        );
    }
}
