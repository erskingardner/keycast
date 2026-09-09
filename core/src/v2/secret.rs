//! Zeroizing wrapper for private material that crosses the API and control boundaries.
//!
//! The wrapper narrows, but does not eliminate, plaintext lifetime: intermediate
//! copies inside third-party HTTP and JSON machinery cannot be erased from here.
//! Browser import therefore remains the weaker path; the trusted CLI stays preferred
//! for sensitive keys.
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroizing;

/// A string whose buffer is erased on drop and never rendered by `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(Zeroizing<String>);

impl Secret {
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    pub fn expose(&self) -> &str {
        self.0.as_str()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn into_zeroizing(self) -> Zeroizing<String> {
        self.0
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Never render private material, including through `{:?}` on a containing struct.
impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Secret(<redacted>)")
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // The deserialized allocation is moved into the zeroizing buffer, not copied.
        String::deserialize(deserializer).map(Self::new)
    }
}

impl Serialize for Secret {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.expose())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_renders_the_value() {
        let secret = Secret::new("nsec1exampleprivatematerial".to_string());
        assert_eq!(format!("{secret:?}"), "Secret(<redacted>)");
        assert!(!format!("{secret:?}").contains("nsec1"));
        assert_eq!(secret.expose(), "nsec1exampleprivatematerial");
    }

    #[test]
    fn round_trips_through_serde_without_exposing_debug() {
        #[derive(Debug, Deserialize, Serialize)]
        struct Request {
            secret_key: Secret,
        }
        let request: Request = serde_json::from_str(r#"{"secret_key":"nsec1abc"}"#).unwrap();
        assert_eq!(request.secret_key.expose(), "nsec1abc");
        assert!(!format!("{request:?}").contains("nsec1abc"));
        assert_eq!(
            serde_json::to_string(&request).unwrap(),
            r#"{"secret_key":"nsec1abc"}"#
        );
    }
}
