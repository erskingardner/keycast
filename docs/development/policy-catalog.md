# Policy use-case catalog

[Development](README.md) · [Policies and access](../policies-and-access.md)

The searchable catalog lives in [catalog.ts](../../web/src/lib/policy/catalog.ts). It contains 74 use cases based on a review of all 99 NIP documents in the NIPs repository and the current Marmot specifications. These are curated permission presets, not end-to-end compatibility certifications for clients.

## Sources and updates

- NIPs snapshot: [`a2494f4f81d4`](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/README.md), reviewed September 10, 2026. This checkout has no nested specification files. External protocols linked from the NIPs README require their own review.
- Marmot snapshot: [`4a2bc65f8db5`](https://github.com/marmot-protocol/marmot/tree/4a2bc65f8db5866cec3b2a127dedb37818eaf207), especially the [Nostr transport](https://github.com/marmot-protocol/marmot/blob/4a2bc65f8db5866cec3b2a127dedb37818eaf207/transports/nostr.md) and [account identity proof](https://github.com/marmot-protocol/marmot/blob/4a2bc65f8db5866cec3b2a127dedb37818eaf207/app-components/account-identity-proof-v2.md).

When updating an entry, verify which key signs each layer, whether the event is unsigned, and which cryptographic methods the hosted identity needs. Keep deprecated kinds optional. Resolve conflicting allocations before adding them. Update this coverage record and the policy editor tests with the catalog.

## Permission semantics

- Templates expand to an explicit union of event kinds and independent cryptographic operations. The strict Rust/TypeScript policy schema is unchanged.
- Template IDs are not saved. Reopening a policy shows its exact kinds under Individual permissions and its saved crypto scopes. It does not infer templates or apply new catalog defaults.
- New templates add their suggestions; removing a template removes only its contribution. Shared kinds remain visible. Manual crypto overrides win over suggestions, including explicit denial.
- NIP-44 and legacy NIP-04 each have separate encrypt/decrypt controls with self-only or any-peer scope. Private list entries need self-only crypto selected separately.
- Kind-only permissions do not restrict content, tags, URLs, deletion targets, app namespaces, wallet commands, or spending amounts. Self-only crypto does not restrict the purpose of encrypted data.
- No signing kind is universally enabled. Private Keycast management kinds 27236/27237 cannot be added; the signer always denies their use through NIP-46.
- New empty policies cannot be saved. Unknown or malformed saved documents cannot be edited, preventing an accidental lossy rewrite.

## Messaging layers

NIP-17 direct messages and Marmot messaging are separate presets in Direct messaging. Both expose gift wraps (1059) as transport handled by the app, not as identity signing grants. NIP-59 seals (13) use the author identity; outer wraps use fresh temporary keys. Unsigned inner rumors also do not need signing permission. Receiving wrapped content uses NIP-44 decryption with the wrapper/seal sender public key.

Marmot grants identity signing for key packages (30443), account proofs (450), welcome seals (13), and inbox relays (10050), plus NIP-44 encrypt/decrypt with any peer. Group-message transport (445) and MLS encryption stay in the client. Optional controls cover read/write relay metadata (10002), push owner proofs (451), and legacy key-package publication (443/10051). The draft multi-device proof (452) is excluded.

## Catalog

Optional capabilities are marked with an asterisk. Encryption suggestions can also be attached to a use case itself (for example, receiving Marmot welcomes) and are always shown in the separate controls.

| Use case | Signing capabilities | Sources |
|---|---|---|
| Notes and replies | Notes and replies (1) | [NIP-01](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/01.md), [NIP-10](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/10.md) |
| Long-form publishing | Published articles (30023); Encrypted drafts (31234); Draft revision history (1234); Private relay preferences * (10013); Legacy article drafts * (30024) | [NIP-23](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/23.md), [NIP-37](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/37.md) |
| Comments | Comments on articles and other content (1111) | [NIP-22](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/22.md) |
| Reactions | Nostr reactions (7); Reactions to websites * (17) | [NIP-25](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/25.md) |
| Reposts | Note reposts (6); Other content reposts (16) | [NIP-18](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/18.md) |
| Profile | Profile name, avatar, and bio (0) | [NIP-01](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/01.md), [NIP-05](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/05.md), [NIP-24](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/24.md) |
| Following and starter packs | Follow list (3); Named follow sets (30000); Starter packs (39089); Media follows * (10020); Media starter packs * (39092) | [NIP-02](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/02.md), [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Profile status | Status updates (30315) | [NIP-38](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/38.md) |
| Relay preferences | Read and write relays (10002); Search relays (10007); Blocked relays (10006); Relay feeds (10012); Named relay sets (30002) | [NIP-65](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/65.md), [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Bookmarks and pins | Bookmarks (10003); Bookmark folders (30003); Pinned notes (10001) | [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Content collections | Article collections (30004); Video collections (30005); Picture collections (30006) | [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Topics and interests | Interests (10015); Topic collections (30015) | [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Custom emoji collections | Favorite emojis (10030); Emoji sets (30030) | [NIP-30](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/30.md), [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Mute lists | Muted accounts and content (10000); Mute sets by kind (30007) | [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| NIP-17 direct messages | Message seals (13); Messaging relay preferences (10050) | [NIP-17](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/17.md), [NIP-59](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/59.md) |
| Chat messages | Chat messages (9) | [NIP-C7](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/C7.md), [NIP-29](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/29.md) |
| Forum threads | Forum threads (11) | [NIP-7D](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/7D.md), [NIP-29](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/29.md) |
| Public messages | Public messages (24) | [NIP-A4](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/A4.md) |
| Marmot messaging | Publish key packages (30443); Authorize MLS account identity (450); Seal welcome invitations (13); Messaging inbox relays (10050); Publish read and write relays * (10002); Push notification owner proofs * (451); Legacy key packages * (443); Legacy key package relays * (10051) | [Marmot](https://github.com/marmot-protocol/marmot/tree/4a2bc65f8db5866cec3b2a127dedb37818eaf207) |
| NIP-59 gift wrapping | Sign message seals (13) | [NIP-59](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/59.md) |
| Group membership | Join groups (9021); Leave groups (9022); Group list (10009) | [NIP-29](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/29.md), [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Picture publishing | Picture posts (20) | [NIP-68](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/68.md) |
| Video publishing | Videos (21); Short videos (22); Addressable videos * (34235); Addressable short videos * (34236) | [NIP-71](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/71.md) |
| Voice messages | Voice messages (1222); Voice replies (1244) | [NIP-A0](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/A0.md) |
| File metadata | File metadata (1063) | [NIP-94](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/94.md) |
| Media server preferences | Blossom servers (10063) | [NIP-B7](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/B7.md), [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Host live events | Live streams (30311); Audio spaces (30312); Meetings (30313) | [NIP-53](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/53.md) |
| Live event participation | Live chat (1311); Room presence (10312) | [NIP-53](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/53.md) |
| Podcast publishing | Show metadata (10154); Episodes (54) | [NIP-F4](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/F4.md) |
| Podcast subscriptions | Favorite podcasts (10054) | [NIP-F4](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/F4.md), [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Calendar publishing | Date events (31922); Timed events (31923); Calendars (31924) | [NIP-52](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/52.md) |
| Calendar RSVPs | RSVPs (31925) | [NIP-52](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/52.md) |
| Create polls | Polls (1068) | [NIP-88](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/88.md) |
| Vote in polls | Poll responses (1018) | [NIP-88](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/88.md) |
| Chess games | Chess game records (64) | [NIP-64](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/64.md) |
| Geocaching | Cache listings (37516); Cache collections (37517); Found logs (7516); Comments (1111) | [NIP-CC](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/CC.md) |
| Wiki editing | Wiki articles (30818); Redirects (30819); Merge proposals (818) | [NIP-54](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/54.md) |
| Wiki source preferences | Preferred authors (10101); Preferred relays (10102) | [NIP-54](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/54.md), [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Text highlights | Highlights (9802) | [NIP-84](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/84.md) |
| Web bookmarks | Web bookmarks (39701) | [NIP-B0](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/B0.md) |
| Code snippets | Code snippets (1337) | [NIP-C0](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/C0.md) |
| Git contributions | Patches (1617); Pull requests (1618); Pull request updates (1619); Issues (1621); Comments (1111) | [NIP-34](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/34.md) |
| Git repository management | Repository announcement (30617); Repository state (30618); Open status (1630); Applied status (1631); Closed status (1632); Draft status (1633) | [NIP-34](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/34.md) |
| Git preferences | Git servers (10317); Followed authors (10017); Followed repositories (10018) | [NIP-34](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/34.md), [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Release artifact collections | Release artifact sets (30063); File metadata (1063) | [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md), [NIP-94](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/94.md) |
| Labels | Content labels (1985) | [NIP-32](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/32.md) |
| Report content | Reports (1984) | [NIP-56](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/56.md) |
| Display badges | Profile badges (10008); Badge collections (30008) | [NIP-58](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/58.md), [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Issue badges | Badge definitions (30009); Badge awards (8) | [NIP-58](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/58.md) |
| App recommendations | Recommended handlers (31989); App collections (30267) | [NIP-89](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/89.md), [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Publish app handlers | App handler metadata (31990) | [NIP-89](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/89.md) |
| Trust provider preferences | Trusted assertion providers (10040) | [NIP-85](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/85.md) |
| Publish trust assertions | User assertions (30382); Event assertions (30383); Address assertions (30384); External identifier assertions (30385) | [NIP-85](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/85.md) |
| Classified listings | Listings (30402); Listing drafts (30403) | [NIP-99](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/99.md) |
| Fundraising goals | Fundraising goals (9041) | [NIP-75](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/75.md) |
| Send zap requests | Zap requests (9734) | [NIP-57](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/57.md) |
| Payment preferences | Payment targets (10133) | [NIP-A3](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/A3.md) |
| Mint recommendations | Mint recommendations (38000) | [NIP-87](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/87.md) |
| Peer-to-peer order announcements | Order announcements (38383) | [NIP-69](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/69.md) |
| Torrent publishing | Torrent metadata (2003); Torrent comments (2004) | [NIP-35](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/35.md) |
| File storage indexes | Root index (15128); Named indexes (35128); Snapshots (5128); Legacy index * (34128) | [NIP-5A](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/5A.md) |
| Application data | Application data (78); Addressable application data (30078) | [NIP-78](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/78.md) |
| HTTP authentication | Sign HTTP authentication (27235) | [NIP-98](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/98.md), [NIP-86](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/86.md) |
| Relay authentication | Sign relay authentication (22242) | [NIP-42](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/42.md), [NIP-70](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/70.md) |
| Relay membership requests | Join relay (28934); Leave relay (28936) | [NIP-43](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/43.md) |
| Delete events | Request event deletion (5) | [NIP-09](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/09.md) |
| Request account removal | Request removal from relays (62) | [NIP-62](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/62.md) |
| Group moderation | Add member or change roles (9000); Remove member (9001); Change metadata (9002); Delete event (9005); Create group (9007); Delete group (9008); Create invitation (9009); Update pinned events (9010) | [NIP-29](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/29.md) |
| Relay monitoring | Monitor announcements (10166); Relay discovery (30166) | [NIP-66](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/66.md) |
| Legacy private messages | Encrypted messages (4) | [NIP-04](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/04.md) |
| Legacy public channels | Create channels (40); Channel metadata (41); Channel messages (42); Hide messages (43); Mute users (44); Channel list (10005) | [NIP-28](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/28.md), [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Legacy communities | Community definitions (34550); Post approvals (4550); Community list (10004) | [NIP-72](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/72.md), [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) |
| Legacy marketplace | Stalls (30017); Products (30018) | [NIP-15](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/15.md) |
| Legacy file server preferences | File server preferences (10096) | [NIP-96](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/96.md) |

## NIP coverage and exclusions

| NIP | Treatment |
| --- | --- |
| [NIP-01](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/01.md) | Mapped to: Notes and replies, Profile. |
| [NIP-02](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/02.md) | Mapped to: Following and starter packs. |
| [NIP-03](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/03.md) | Unrecommended timestamp attestations (1040); exclude from ordinary presets. |
| [NIP-04](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/04.md) | Legacy messages and separate encryption/decryption controls. |
| [NIP-05](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/05.md) | Mapped to: Profile. |
| [NIP-06](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/06.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-07](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/07.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-08](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/08.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-09](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/09.md) | Mapped to: Delete events. |
| [NIP-10](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/10.md) | Mapped to: Notes and replies. |
| [NIP-11](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/11.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-12](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/12.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-13](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/13.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-14](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/14.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-15](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/15.md) | Mapped to: Legacy marketplace. |
| [NIP-16](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/16.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-17](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/17.md) | Mapped to: Private messages. |
| [NIP-18](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/18.md) | Mapped to: Reposts. |
| [NIP-19](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/19.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-20](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/20.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-21](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/21.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-22](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/22.md) | Mapped to: Comments. |
| [NIP-23](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/23.md) | Mapped to: Long-form publishing. |
| [NIP-24](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/24.md) | Mapped to: Profile. |
| [NIP-25](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/25.md) | Mapped to: Reactions. |
| [NIP-26](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/26.md) | Unrecommended delegation requires raw signatures; no normal event preset. |
| [NIP-27](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/27.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-28](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/28.md) | Mapped to: Legacy public channels. |
| [NIP-29](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/29.md) | Mapped to: Chat messages, Forum threads, Group membership, Group moderation. |
| [NIP-30](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/30.md) | Mapped to: Custom emoji collections. |
| [NIP-31](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/31.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-32](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/32.md) | Mapped to: Labels. |
| [NIP-33](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/33.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-34](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/34.md) | Mapped to: Git contributions, Git repository management, Git preferences. |
| [NIP-35](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/35.md) | Mapped to: Torrent publishing. |
| [NIP-36](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/36.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-37](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/37.md) | Mapped to: Long-form publishing. |
| [NIP-38](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/38.md) | Mapped to: Profile status. |
| [NIP-39](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/39.md) | Withheld: kind 10011 conflicts with favorite follow sets in NIP-51. |
| [NIP-40](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/40.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-42](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/42.md) | Mapped to: Relay authentication. |
| [NIP-43](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/43.md) | User join/leave 28934/28936. Relay-owned membership/role/invite events are not ordinary user capabilities. |
| [NIP-44](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/44.md) | Separate encrypt and decrypt capabilities, with self-only or any-peer scope. |
| [NIP-45](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/45.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-46](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/46.md) | Signer transport (24133), not a capability of the hosted identity. Public-key lookup and protocol housekeeping are separate from signing grants. |
| [NIP-47](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/47.md) | Wallet connection client uses a dedicated connection key (23194), wallet service uses 13194/23195. Do not bundle these with social identity. Requires separate role and client validation; kind policy cannot limit wallet commands or spend amounts. |
| [NIP-48](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/48.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-49](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/49.md) | Private-key password encryption format, not a remote signer permission. |
| [NIP-50](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/50.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-51](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/51.md) | Collections mapped above. Private entries optionally require NIP-44 self encrypt/decrypt; NIP-04 only for legacy data. 10011 conflict withheld; 10064 authorship pending. Deprecated 30001 not default. |
| [NIP-52](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/52.md) | Mapped to: Calendar publishing, Calendar RSVPs. |
| [NIP-53](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/53.md) | Mapped to: Host live events, Live event participation. |
| [NIP-54](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/54.md) | Mapped to: Wiki editing, Wiki source preferences. |
| [NIP-55](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/55.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-56](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/56.md) | Mapped to: Report content. |
| [NIP-57](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/57.md) | User request 9734; receipt 9735 signed by LNURL service, excluded from user template. |
| [NIP-58](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/58.md) | Mapped to: Display badges, Issue badges. |
| [NIP-59](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/59.md) | Message seals use the sender key (13); gift wrappers (1059/21059) use fresh keys. Inner messages (14/15/7) are unsigned. No blanket grant for every layer. |
| [NIP-5A](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/5A.md) | Mapped to: File storage indexes. |
| [NIP-60](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/60.md) | Advanced wallet design pending: 17375, 7375, 7376, optional 7374, deletion 5, NIP-44 self encrypt/decrypt. Decryption can reveal bearer proofs and wallet keys; kind-only grants cannot isolate wallet objects. |
| [NIP-61](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/61.md) | Advanced ecash design pending: nutzaps 9321, receive config 10019, wallet history 7376. Redemption spending key differs from social key; validate with NIP-60 clients. |
| [NIP-62](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/62.md) | Mapped to: Request account removal. |
| [NIP-64](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/64.md) | Mapped to: Chess games. |
| [NIP-65](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/65.md) | Mapped to: Relay preferences. |
| [NIP-66](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/66.md) | Mapped to: Relay monitoring. |
| [NIP-67](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/67.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-68](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/68.md) | Mapped to: Picture publishing. |
| [NIP-69](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/69.md) | Mapped to: Peer-to-peer order announcements. |
| [NIP-70](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/70.md) | Mapped to: Relay authentication. |
| [NIP-71](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/71.md) | Mapped to: Video publishing. |
| [NIP-72](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/72.md) | Mapped to: Legacy communities. |
| [NIP-73](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/73.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-75](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/75.md) | Mapped to: Fundraising goals. |
| [NIP-77](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/77.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-78](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/78.md) | Mapped to: Application data. |
| [NIP-7D](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/7D.md) | Mapped to: Forum threads. |
| [NIP-84](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/84.md) | Mapped to: Text highlights. |
| [NIP-85](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/85.md) | Mapped to: Trust provider preferences, Publish trust assertions. |
| [NIP-86](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/86.md) | Mapped to: HTTP authentication. |
| [NIP-87](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/87.md) | User recommendations 38000; mint announcements 38172/38173 belong to mint operators and are excluded from normal identity presets. |
| [NIP-88](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/88.md) | Mapped to: Create polls, Vote in polls. |
| [NIP-89](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/89.md) | Mapped to: App recommendations, Publish app handlers. |
| [NIP-90](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/90.md) | Unrecommended legacy DVM protocol. Requests 5000–5999, results 6000–6999, feedback 7000. Do not enable entire ranges; enumerate task and client/provider role after application validation. |
| [NIP-92](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/92.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-94](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/94.md) | Mapped to: File metadata, Release artifact collections. |
| [NIP-96](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/96.md) | Mapped to: Legacy file server preferences. |
| [NIP-98](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/98.md) | Mapped to: HTTP authentication. |
| [NIP-99](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/99.md) | Mapped to: Classified listings. |
| [NIP-A0](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/A0.md) | Mapped to: Voice messages. |
| [NIP-A3](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/A3.md) | Mapped to: Payment preferences. |
| [NIP-A4](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/A4.md) | Mapped to: Public messages. |
| [NIP-B0](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/B0.md) | Mapped to: Web bookmarks. |
| [NIP-B7](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/B7.md) | Media server preferences 10063. Blossom operations and auth 24242 are defined in external BUDs; separate external-spec review required before an upload/delete preset. |
| [NIP-BE](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/BE.md) | Transport, encoding, metadata/tags, discovery, client behavior, or superseded specification; no additional standalone signing permission. |
| [NIP-C0](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/C0.md) | Mapped to: Code snippets. |
| [NIP-C7](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/C7.md) | Mapped to: Chat messages. |
| [NIP-CC](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/CC.md) | Mapped to: Geocaching. |
| [NIP-EE](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/EE.md) | Deprecated NIP-EE moved to external Marmot specification. The Marmot preset uses the current external transport and account-proof documents; 443/10051 are optional legacy compatibility only. |
| [NIP-F4](https://github.com/nostr-protocol/nips/blob/a2494f4f81d46684e5814a9bf35e2b1df978f955/F4.md) | Podcast key: 10154/54; listener 10054. Authorship withheld: prose says 10164 while example and NIP-51 say 10064. |


## Validation

`web/src/lib/policy/editor.test.ts` covers every catalog preset round trip, exact saved permissions, shared kinds, dependency narrowing, explicit overrides, unknown documents, and NIP-17/Marmot signing boundaries. Browser validation covers search and keyboard behavior, create/edit requests with signed approval bodies, encryption-only policies, custom kinds, and mobile layout. These checks do not certify compatibility with every client listed by an upstream NIP.
