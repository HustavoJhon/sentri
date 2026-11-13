//! Conversion between domain timestamps and INTEGER unix seconds.

use chrono::{DateTime, Utc};

use crate::StorageError;

pub(crate) fn to_db(value: DateTime<Utc>) -> i64 {
    value.timestamp()
}

pub(crate) fn opt_to_db(value: Option<DateTime<Utc>>) -> Option<i64> {
    value.map(to_db)
}

pub(crate) fn from_db(value: i64, column: &'static str) -> Result<DateTime<Utc>, StorageError> {
    DateTime::from_timestamp(value, 0).ok_or(StorageError::Corrupt {
        column,
        value: value.to_string(),
    })
}

pub(crate) fn opt_from_db(
    value: Option<i64>,
    column: &'static str,
) -> Result<Option<DateTime<Utc>>, StorageError> {
    value.map(|v| from_db(v, column)).transpose()
}
