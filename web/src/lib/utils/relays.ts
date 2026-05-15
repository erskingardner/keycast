export const REQUIRED_PUBLIC_RELAYS = [
    "wss://nos.lol",
    "wss://relay.primal.net",
    "wss://relay.ditto.pub",
] as const;

export const DEFAULT_NOSTR_READ_RELAYS = [
    "wss://purplepag.es",
    ...REQUIRED_PUBLIC_RELAYS,
    "wss://relay.snort.social",
    "wss://relay.damus.io",
] as const;

export const DEFAULT_OUTBOX_RELAYS = [...REQUIRED_PUBLIC_RELAYS] as const;

export const DEFAULT_AUTHORIZATION_RELAYS = [...REQUIRED_PUBLIC_RELAYS] as const;

export function relayListForInput(
    relays: readonly string[] = DEFAULT_AUTHORIZATION_RELAYS,
): string {
    return relays.join(", ");
}

export function parseRelayInput(input: string): string[] {
    return input
        .split(",")
        .map((relay) => relay.trim())
        .filter(Boolean);
}
