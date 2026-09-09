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

/// Substrings that must never reach an approval event an external signer
/// displays, stores, or transports over relays.
const SECRET_MARKERS: [&str; 2] = ["secret_key", "nsec1"];

/// Deserializing only the name lets serde skip the private field without
/// allocating it, unlike parsing the whole body into a `Value`.
#[derive(serde::Deserialize)]
struct ImportedKeyName {
    name: String,
}

/// Human-readable, verified command contents for the external approval screen.
/// Private import material must never be copied into an approval event.
pub fn description(method: &str, path: &str, body: &str) -> Option<String> {
    if method == "POST" && path.split('?').next()?.ends_with("/keys") {
        let request: ImportedKeyName = serde_json::from_str(body).ok()?;
        Some(format!("Import private key named {}", request.name))
    } else {
        Some(body.to_owned())
    }
}

/// Fail closed instead of letting a route echo private material into an approval.
/// Guards both the redacted import summary and any future write whose body is echoed.
pub fn contains_secret_marker(content: &str) -> bool {
    let lowered = content.to_ascii_lowercase();
    SECRET_MARKERS
        .iter()
        .any(|marker| lowered.contains(*marker))
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
    /// The browser refuses to sign first, so a marker present only here would let
    /// content the signer rejects still reach an external key store. The list is
    /// read from the TypeScript source rather than duplicated in this test.
    #[test]
    fn secret_markers_match_the_browser_list() {
        let typescript = include_str!("../../../web/src/lib/utils/management.ts");
        let declaration = typescript
            .lines()
            .find(|line| line.contains("const SECRET_MARKERS"))
            .expect("web/src/lib/utils/management.ts declares SECRET_MARKERS");
        let browser: Vec<&str> = declaration.split('"').skip(1).step_by(2).collect();
        assert_eq!(
            browser, SECRET_MARKERS,
            "the browser and signer marker lists have diverged"
        );
    }

    #[test]
    fn import_description_names_the_key_without_its_private_material() {
        let body = r#"{"name":"Personal identity","secret_key":"nsec1exampleprivatematerial"}"#;
        let description = description("POST", "/teams/1/keys", body).unwrap();
        assert_eq!(description, "Import private key named Personal identity");
        assert!(!description.contains("nsec1"));
        assert!(!contains_secret_marker(&description));
    }

    #[test]
    fn secret_markers_are_detected_in_echoed_bodies_and_pasted_names() {
        // A future route that echoes its body must not ship private material.
        let echoed = description("PUT", "/teams/1", r#"{"secret_key":"nsec1abc"}"#).unwrap();
        assert!(contains_secret_marker(&echoed));
        // An operator pasting a private key into the name field is caught too.
        let pasted = description(
            "POST",
            "/teams/1/keys",
            r#"{"name":"NSEC1abc","secret_key":"x"}"#,
        )
        .unwrap();
        assert!(contains_secret_marker(&pasted));
        assert!(!contains_secret_marker(r#"{"name":"Ops"}"#));
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
