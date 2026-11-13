use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ParseError;

/// How an opportunity takes place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    Online,
    InPerson,
    Hybrid,
    #[default]
    Unknown,
}

impl Modality {
    pub const ALL: [Self; 4] = [Self::Online, Self::InPerson, Self::Hybrid, Self::Unknown];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::InPerson => "in_person",
            Self::Hybrid => "hybrid",
            Self::Unknown => "unknown",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Online => "Online",
            Self::InPerson => "In person",
            Self::Hybrid => "Hybrid",
            Self::Unknown => "Unknown",
        }
    }

    /// Whether the opportunity can be attended remotely.
    pub const fn is_remote_friendly(self) -> bool {
        matches!(self, Self::Online | Self::Hybrid)
    }
}

impl fmt::Display for Modality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl FromStr for Modality {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '-'], "_")
            .as_str()
        {
            "online" | "virtual" | "remote" => Ok(Self::Online),
            "in_person" | "presencial" | "onsite" | "on_site" => Ok(Self::InPerson),
            "hybrid" | "hibrido" | "híbrido" => Ok(Self::Hybrid),
            "unknown" | "" => Ok(Self::Unknown),
            _ => Err(ParseError::new("modality", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aliases() {
        assert_eq!("Remote".parse(), Ok(Modality::Online));
        assert_eq!("presencial".parse(), Ok(Modality::InPerson));
        assert_eq!("on-site".parse(), Ok(Modality::InPerson));
        for m in Modality::ALL {
            assert_eq!(m.as_str().parse(), Ok(m));
        }
    }
}
