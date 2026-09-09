export const REQUIRED_PUBLIC_RELAYS = [
    "wss://nos.lol",
    "wss://relay.primal.net",
    "wss://relay.damus.io",
] as const;

export const DEFAULT_NOSTR_READ_RELAYS = [
    "wss://purplepag.es",
    ...REQUIRED_PUBLIC_RELAYS,
    "wss://relay.snort.social",
] as const;

export const DEFAULT_OUTBOX_RELAYS = [...REQUIRED_PUBLIC_RELAYS] as const;
