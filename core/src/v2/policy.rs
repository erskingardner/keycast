use std::collections::BTreeSet;

use nostr::prelude::PublicKey;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyDocument {
    pub version: u16,
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sign_event: Option<SignEventCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nip04_encrypt: Option<CryptoCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nip04_decrypt: Option<CryptoCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nip44_encrypt: Option<CryptoCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nip44_decrypt: Option<CryptoCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignEventCapability {
    pub allowed_kinds: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CryptoCapability {
    pub recipient: RecipientScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipientScope {
    SelfOnly,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestedCapabilities(Option<BTreeSet<String>>);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyError {
    #[error("unsupported policy version")]
    UnsupportedVersion,
    #[error("policy must grant at least one capability")]
    Empty,
    #[error("sign_event.allowed_kinds must contain at least one kind")]
    EmptyKinds,
    #[error("sign_event.allowed_kinds contains a duplicate kind")]
    DuplicateKind,
    #[error("invalid requested capability: {0}")]
    InvalidRequestedCapability(String),
    #[error("invalid policy JSON: {0}")]
    InvalidJson(String),
}

impl PolicyDocument {
    pub fn parse(json: &str) -> Result<Self, PolicyError> {
        let policy: Self =
            serde_json::from_str(json).map_err(|e| PolicyError::InvalidJson(e.to_string()))?;
        policy.validate()?;
        Ok(policy)
    }

    pub fn validate(&self) -> Result<(), PolicyError> {
        if self.version != 1 {
            return Err(PolicyError::UnsupportedVersion);
        }

        let capabilities = &self.capabilities;
        if capabilities.sign_event.is_none()
            && capabilities.nip04_encrypt.is_none()
            && capabilities.nip04_decrypt.is_none()
            && capabilities.nip44_encrypt.is_none()
            && capabilities.nip44_decrypt.is_none()
        {
            return Err(PolicyError::Empty);
        }

        if let Some(sign) = &capabilities.sign_event {
            if sign.allowed_kinds.is_empty() {
                return Err(PolicyError::EmptyKinds);
            }
            let unique: BTreeSet<u16> = sign.allowed_kinds.iter().copied().collect();
            if unique.len() != sign.allowed_kinds.len() {
                return Err(PolicyError::DuplicateKind);
            }
        }

        Ok(())
    }

    pub fn allows_sign_event(&self, kind: u16, requested: &RequestedCapabilities) -> bool {
        kind != super::management::MANAGEMENT_KIND
            && kind != super::management::MANAGEMENT_READ_KIND
            && self
                .capabilities
                .sign_event
                .as_ref()
                .is_some_and(|capability| capability.allowed_kinds.contains(&kind))
            && requested.allows_sign_kind(kind)
    }

    pub fn allows_crypto(
        &self,
        method: &str,
        recipient: &PublicKey,
        user: &PublicKey,
        requested: &RequestedCapabilities,
    ) -> bool {
        let capability = match method {
            "nip04_encrypt" => self.capabilities.nip04_encrypt.as_ref(),
            "nip04_decrypt" => self.capabilities.nip04_decrypt.as_ref(),
            "nip44_encrypt" => self.capabilities.nip44_encrypt.as_ref(),
            "nip44_decrypt" => self.capabilities.nip44_decrypt.as_ref(),
            _ => None,
        };

        capability.is_some_and(|capability| {
            matches!(capability.recipient, RecipientScope::Any) || recipient == user
        }) && requested.allows_method(method)
    }
}

impl RequestedCapabilities {
    pub fn unrestricted() -> Self {
        Self(None)
    }

    pub fn parse(raw: Option<&str>) -> Result<Self, PolicyError> {
        let Some(raw) = raw.filter(|value| !value.trim().is_empty()) else {
            return Ok(Self::unrestricted());
        };

        let mut capabilities = BTreeSet::new();
        for value in raw.split(',').map(str::trim) {
            if !is_valid_requested_capability(value) || !capabilities.insert(value.to_owned()) {
                return Err(PolicyError::InvalidRequestedCapability(value.to_owned()));
            }
        }
        Ok(Self(Some(capabilities)))
    }

    pub fn from_json(json: Option<&str>) -> Result<Self, PolicyError> {
        let Some(json) = json else {
            return Ok(Self::unrestricted());
        };
        let values: Vec<String> =
            serde_json::from_str(json).map_err(|e| PolicyError::InvalidJson(e.to_string()))?;
        if values.is_empty() {
            return Ok(Self(Some(BTreeSet::new())));
        }
        Self::parse(Some(&values.join(",")))
    }

    pub fn as_json(&self) -> Result<Option<String>, serde_json::Error> {
        self.0
            .as_ref()
            .map(|values| serde_json::to_string(&values.iter().collect::<Vec<_>>()))
            .transpose()
    }

    fn allows_method(&self, method: &str) -> bool {
        self.0
            .as_ref()
            .is_none_or(|capabilities| capabilities.contains(method))
    }

    fn allows_sign_kind(&self, kind: u16) -> bool {
        self.0.as_ref().is_none_or(|capabilities| {
            capabilities.contains("sign_event")
                || capabilities.contains(&format!("sign_event:{kind}"))
        })
    }
}

fn is_valid_requested_capability(value: &str) -> bool {
    matches!(
        value,
        "sign_event" | "nip04_encrypt" | "nip04_decrypt" | "nip44_encrypt" | "nip44_decrypt"
    ) || value
        .strip_prefix("sign_event:")
        .is_some_and(|kind| kind.parse::<u16>().is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> PolicyDocument {
        PolicyDocument {
            version: 1,
            capabilities: Capabilities {
                sign_event: Some(SignEventCapability {
                    allowed_kinds: vec![1, 7],
                }),
                nip44_encrypt: Some(CryptoCapability {
                    recipient: RecipientScope::SelfOnly,
                }),
                ..Capabilities::default()
            },
        }
    }

    #[test]
    fn unknown_fields_and_empty_policies_fail_closed() {
        assert!(
            PolicyDocument::parse(r#"{"version":1,"capabilities":{},"allow_all":true}"#).is_err()
        );
        assert_eq!(
            policy_with_no_capabilities().validate(),
            Err(PolicyError::Empty)
        );
    }

    #[test]
    fn requested_permissions_only_narrow_policy() {
        let requested = RequestedCapabilities::parse(Some("sign_event:1")).unwrap();
        assert!(policy().allows_sign_event(1, &requested));
        assert!(!policy().allows_sign_event(7, &requested));
        assert!(!policy().allows_sign_event(2, &RequestedCapabilities::unrestricted()));
    }

    #[test]
    fn duplicate_and_unknown_requested_permissions_are_rejected() {
        assert!(RequestedCapabilities::parse(Some("sign_event,sign_event")).is_err());
        assert!(RequestedCapabilities::parse(Some("admin")).is_err());
        assert!(RequestedCapabilities::parse(Some("sign_event:70000")).is_err());
    }

    fn policy_with_no_capabilities() -> PolicyDocument {
        PolicyDocument {
            version: 1,
            capabilities: Capabilities::default(),
        }
    }
}
