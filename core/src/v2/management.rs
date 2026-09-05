//! Private Keycast management protocol, not a new Nostr standard.
use nostr::prelude::{Event, PublicKey};

/// Hard denied on every delegated sign_event path, independent of policy.
pub const MANAGEMENT_KIND: u16 = 27236;
/// Instance-bound management reads must not be delegated through ordinary NIP-98.
pub const MANAGEMENT_READ_KIND: u16 = 27237;
pub const MAX_HTTP_BODY: usize = 1024 * 1024;
pub const MAX_CONTROL_BYTES: u64 = 8 * 1024 * 1024;

pub fn exact_tag<'a>(event: &'a Event, name: &str) -> Option<&'a str> {
    let mut tags = event.tags.iter().filter(|t| t.kind() == name);
    let tag = tags.next()?;
    if tags.next().is_some() || tag.as_slice().len() != 2 {
        return None;
    }
    tag.content()
}

pub fn approval_context(event: &Event, instance: &str, revision: i64) -> Option<PublicKey> {
    if event.kind.as_u16() != MANAGEMENT_KIND
        || exact_tag(event, "instance")? != instance
        || exact_tag(event, "revision")? != revision.to_string()
    {
        return None;
    }
    let nonce = exact_tag(event, "nonce")?;
    if nonce.len() != 64 || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    PublicKey::from_hex(exact_tag(event, "response")?).ok()
}

/// Human-readable, verified command contents for the external approval screen.
/// Private import material must never be copied into an approval event.
pub fn description(method: &str, path: &str, body: &str) -> Option<String> {
    if method == "POST" && path.split('?').next()?.ends_with("/keys") {
        let value: serde_json::Value = serde_json::from_str(body).ok()?;
        Some(format!(
            "Import private key named {}",
            value.get("name")?.as_str()?
        ))
    } else {
        Some(body.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::prelude::*;
    fn approval(tags: Vec<Tag>) -> Event {
        EventBuilder::new(Kind::Custom(MANAGEMENT_KIND), "command")
            .tags(tags)
            .finalize(&Keys::generate())
            .unwrap()
    }
    #[test]
    fn approval_context_rejects_cross_instance_revision_and_ambiguous_tags() {
        let response = Keys::generate().public_key();
        let tags = vec![
            Tag::parse(["instance", "original"]).unwrap(),
            Tag::parse(["revision", "7"]).unwrap(),
            Tag::parse(["nonce", &"a".repeat(64)]).unwrap(),
            Tag::parse(["response", &response.to_hex()]).unwrap(),
        ];
        let valid = approval(tags.clone());
        assert_eq!(approval_context(&valid, "original", 7), Some(response));
        assert!(approval_context(&valid, "restored", 7).is_none());
        assert!(approval_context(&valid, "original", 8).is_none());
        for i in 0..tags.len() {
            let mut missing = tags.clone();
            missing.remove(i);
            assert!(approval_context(&approval(missing), "original", 7).is_none());
            let mut duplicate = tags.clone();
            duplicate.push(Tag::parse([tags[i].kind().to_string().as_str(), "different"]).unwrap());
            assert!(approval_context(&approval(duplicate), "original", 7).is_none());
            let mut malformed = tags.clone();
            malformed[i] = Tag::parse([
                tags[i].kind().to_string().as_str(),
                tags[i].content().unwrap(),
                "extra",
            ])
            .unwrap();
            assert!(approval_context(&approval(malformed), "original", 7).is_none());
        }
    }
}
