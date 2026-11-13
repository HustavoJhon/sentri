use thiserror::Error;

/// Error returned when a textual value cannot be converted into a domain type.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("invalid {kind}: {value:?}")]
pub struct ParseError {
    pub kind: &'static str,
    pub value: String,
}

impl ParseError {
    pub(crate) fn new(kind: &'static str, value: &str) -> Self {
        Self {
            kind,
            value: value.to_owned(),
        }
    }
}
