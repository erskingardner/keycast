import { afterEach, describe, expect, test } from "bun:test";
import type { EventTemplate, NostrEvent } from "applesauce-core/helpers";
import {
    buildNip46SigningPermissions,
    clearActiveSigner,
    createNostrConnectSigninSession,
    getActiveSignerSummary,
    npubForPubkey,
    normalizeBunkerUri,
    setActiveSigner,
    signNostrEvent,
} from "$lib/nostr";
import { truncatedNpubForPubkey, userFromPubkeyOrNpub } from "./nostr";

const PUBKEY =
    "0000000000000000000000000000000000000000000000000000000000000001";
const OTHER_PUBKEY =
    "0000000000000000000000000000000000000000000000000000000000000002";

afterEach(() => {
    clearActiveSigner();
});

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

    test("normalizes bunker URIs before connecting remote signers", () => {
        expect(normalizeBunkerUri("  bunker://abc?relay=wss://relay.example  ")).toBe(
            "bunker://abc?relay=wss://relay.example",
        );

        expect(() => normalizeBunkerUri("nostrconnect://abc")).toThrow(
            "Paste a bunker:// remote signer connection string",
        );
    });

    test("requests NIP-46 permission to sign NIP-98 HTTP auth events", () => {
        expect(buildNip46SigningPermissions()).toEqual([
            "get_public_key",
            "sign_event:27235",
        ]);
    });

    test("creates Amber sign-in sessions through Nostr Connect", () => {
        const session = createNostrConnectSigninSession({ signerKind: "amber" });
        const uri = new URL(session.uri);

        session.cancel();

        expect(uri.protocol).toBe("nostrconnect:");
        expect(uri.searchParams.get("perms")).toBe(
            "get_public_key,sign_event:27235",
        );
    });

    test("signs events with the active signer", async () => {
        const template = authTemplate();
        const signedEvent = signedAuthEvent(PUBKEY);

        setActiveSigner({
            kind: "extension",
            signer: {
                getPublicKey: async () => PUBKEY,
                signEvent: async () => signedEvent,
            },
            pubkey: PUBKEY,
        });

        await expect(signNostrEvent(template, PUBKEY)).resolves.toEqual(signedEvent);
        expect(getActiveSignerSummary()).toEqual({
            kind: "extension",
            pubkey: PUBKEY,
        });
    });

    test("rejects events signed by a different pubkey", async () => {
        setActiveSigner({
            kind: "amber",
            signer: {
                getPublicKey: async () => PUBKEY,
                signEvent: async () => signedAuthEvent(OTHER_PUBKEY),
            },
            pubkey: PUBKEY,
        });

        await expect(signNostrEvent(authTemplate(), PUBKEY)).rejects.toThrow(
            "signed with a different pubkey",
        );
    });
});

function authTemplate(): EventTemplate {
    return {
        kind: 27235,
        created_at: 1,
        content: "",
        tags: [["method", "GET"]],
    };
}

function signedAuthEvent(pubkey: string): NostrEvent {
    return {
        ...authTemplate(),
        id: `event-${pubkey}`,
        pubkey,
        sig: `sig-${pubkey}`,
    };
}
