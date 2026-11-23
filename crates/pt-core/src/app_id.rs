use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A Steam application ID.
///
/// The value is always a positive integer. Parsing is deliberately strict
/// (ASCII digits only) because an `AppId` is later passed as an argument to
/// external tools such as `protontricks-launch`; a value like `--help` or
/// `+5` must never get that far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct AppId(u32);

/// Reasons an [`AppId`] could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AppIdError {
    /// The input was not a plain decimal number that fits in `u32`.
    #[error("invalid Steam AppID {0:?}: expected a positive decimal integer")]
    Invalid(String),
    /// Zero is not a valid AppID.
    #[error("Steam AppID cannot be zero")]
    Zero,
}

impl AppId {
    /// Builds an `AppId` from a number, rejecting zero.
    pub fn new(value: u32) -> Result<Self, AppIdError> {
        if value == 0 {
            Err(AppIdError::Zero)
        } else {
            Ok(Self(value))
        }
    }

    /// Returns the numeric value.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for AppId {
    type Error = AppIdError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AppId> for u32 {
    fn from(value: AppId) -> Self {
        value.0
    }
}

impl FromStr for AppId {
    type Err = AppIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(AppIdError::Invalid(s.to_owned()));
        }
        let value: u32 = s.parse().map_err(|_| AppIdError::Invalid(s.to_owned()))?;
        Self::new(value)
    }
}

impl fmt::Display for AppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_decimal() {
        assert_eq!("1593500".parse::<AppId>().unwrap().get(), 1_593_500);
    }

    #[test]
    fn accepts_leading_zeros_as_the_same_number() {
        assert_eq!("007".parse::<AppId>().unwrap(), AppId::new(7).unwrap());
    }

    #[test]
    fn rejects_zero() {
        assert_eq!("0".parse::<AppId>(), Err(AppIdError::Zero));
        assert_eq!(AppId::new(0), Err(AppIdError::Zero));
    }

    #[test]
    fn rejects_anything_that_is_not_plain_digits() {
        for input in [
            "", " 5", "5 ", "+5", "-5", "5.0", "0x10", "--help", "12abc", "１２",
        ] {
            assert!(
                matches!(input.parse::<AppId>(), Err(AppIdError::Invalid(_))),
                "{input:?} should be rejected"
            );
        }
    }

    #[test]
    fn rejects_values_that_overflow_u32() {
        assert!(matches!(
            "4294967296".parse::<AppId>(),
            Err(AppIdError::Invalid(_))
        ));
        assert_eq!("4294967295".parse::<AppId>().unwrap().get(), u32::MAX);
    }

    #[test]
    fn display_round_trips_through_from_str() {
        let id = AppId::new(1593500).unwrap();
        assert_eq!(id.to_string().parse::<AppId>().unwrap(), id);
    }

    #[test]
    fn serde_uses_a_plain_number_and_validates_on_input() {
        let id = AppId::new(42).unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), "42");
        assert_eq!(serde_json::from_str::<AppId>("42").unwrap(), id);
        assert!(serde_json::from_str::<AppId>("0").is_err());
        assert!(serde_json::from_str::<AppId>("\"42\"").is_err());
    }
}
