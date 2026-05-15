import { describe, expect, test } from "bun:test";
import {
    DEFAULT_AUTHORIZATION_RELAYS,
    DEFAULT_NOSTR_READ_RELAYS,
    DEFAULT_OUTBOX_RELAYS,
    relayListForInput,
    parseRelayInput,
} from "./relays";

const DEAD_RELAYS = [
    ["relay", "nsec" + "bunker", "com"].join("."),
    ["relay", "nostr", "band"].join("."),
];
const REQUIRED_RELAYS = [
    "wss://nos.lol",
    "wss://relay.primal.net",
    "wss://relay.ditto.pub",
] as const;

describe("relay defaults", () => {
    test("do not include dead relay hosts", () => {
        const defaults = [
            ...DEFAULT_NOSTR_READ_RELAYS,
            ...DEFAULT_OUTBOX_RELAYS,
            ...DEFAULT_AUTHORIZATION_RELAYS,
        ];

        for (const deadRelay of DEAD_RELAYS) {
            expect(defaults.some((relay) => relay.includes(deadRelay))).toBe(false);
        }
    });

    test("include the current public relay replacements", () => {
        for (const requiredRelay of REQUIRED_RELAYS) {
            expect(DEFAULT_AUTHORIZATION_RELAYS).toContain(requiredRelay);
            expect(DEFAULT_NOSTR_READ_RELAYS).toContain(requiredRelay);
        }
    });

    test("formats and parses comma-separated authorization relay input", () => {
        expect(relayListForInput()).toBe(
            "wss://nos.lol, wss://relay.primal.net, wss://relay.ditto.pub",
        );
        expect(parseRelayInput(" wss://nos.lol, ,wss://relay.primal.net ")).toEqual([
            "wss://nos.lol",
            "wss://relay.primal.net",
        ]);
    });
});
