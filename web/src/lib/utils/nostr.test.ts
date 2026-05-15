import { describe, expect, test } from "bun:test";
import { npubForPubkey } from "$lib/nostr";
import { truncatedNpubForPubkey, userFromPubkeyOrNpub } from "./nostr";

const PUBKEY =
    "0000000000000000000000000000000000000000000000000000000000000001";

describe("Nostr helper utilities", () => {
    test("creates app users from hex pubkeys", () => {
        const npub = npubForPubkey(PUBKEY);
        expect(npub).toBeTruthy();

        expect(userFromPubkeyOrNpub(PUBKEY)).toEqual({
            pubkey: PUBKEY,
            npub: npub!,
        });
    });

    test("creates app users from npubs", () => {
        const npub = npubForPubkey(PUBKEY);

        expect(npub?.startsWith("npub1")).toBe(true);
        expect(userFromPubkeyOrNpub(npub || "")?.pubkey).toBe(PUBKEY);
    });

    test("rejects invalid pubkey input", () => {
        expect(userFromPubkeyOrNpub("not-a-pubkey")).toBeNull();
    });

    test("truncates npub output", () => {
        const npub = npubForPubkey(PUBKEY);

        expect(truncatedNpubForPubkey(PUBKEY, 12)).toBe(npub?.slice(0, 12));
    });
});
