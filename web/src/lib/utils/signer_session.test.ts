import { describe, expect, test } from "bun:test";
import {
    parseStoredSignerSession,
    serializeStoredSignerSession,
} from "./signer_session";

const PUBKEY = "6f".repeat(32);
const CLIENT_SECRET = "a1".repeat(32);

describe("signer session storage", () => {
    test("round-trips extension and Amber signer sessions", () => {
        expect(
            parseStoredSignerSession(
                serializeStoredSignerSession({
                    kind: "extension",
                    pubkey: PUBKEY.toUpperCase(),
                }),
            ),
        ).toEqual({ kind: "extension", pubkey: PUBKEY });

        expect(
            parseStoredSignerSession(
                serializeStoredSignerSession({
                    kind: "amber",
                    pubkey: PUBKEY,
                }),
            ),
        ).toEqual({ kind: "amber", pubkey: PUBKEY });
    });

    test("round-trips remote signer session metadata", () => {
        expect(
            parseStoredSignerSession(
                serializeStoredSignerSession({
                    kind: "remote",
                    pubkey: PUBKEY,
                    bunkerUri: "bunker://abc?relay=wss://relay.example",
                    clientSecretHex: CLIENT_SECRET.toUpperCase(),
                }),
            ),
        ).toEqual({
            kind: "remote",
            pubkey: PUBKEY,
            bunkerUri: "bunker://abc?relay=wss://relay.example",
            clientSecretHex: CLIENT_SECRET,
        });
    });

    test("fails closed for malformed sessions", () => {
        expect(parseStoredSignerSession("{")).toBeNull();
        expect(parseStoredSignerSession("{}")).toBeNull();
        expect(
            parseStoredSignerSession(
                JSON.stringify({ kind: "amber", pubkey: "not-a-key" }),
            ),
        ).toBeNull();
        expect(
            parseStoredSignerSession(
                JSON.stringify({
                    kind: "remote",
                    pubkey: PUBKEY,
                    bunkerUri: "nostrconnect://abc",
                    clientSecretHex: CLIENT_SECRET,
                }),
            ),
        ).toBeNull();
    });
});
