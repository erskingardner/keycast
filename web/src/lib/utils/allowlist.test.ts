import { describe, expect, test } from "bun:test";
import {
    fetchPubkeyAllowlist,
    isPubkeyAllowed,
    parsePubkeyAllowlist,
} from "./allowlist";

describe("pubkey allowlist helpers", () => {
    test("treats blank allowlists as open", () => {
        expect(isPubkeyAllowed("abc", "")).toBe(true);
        expect(isPubkeyAllowed("abc", " , ")).toBe(true);
        expect(isPubkeyAllowed("abc", undefined)).toBe(true);
    });

    test("trims entries and requires exact matches", () => {
        expect(parsePubkeyAllowlist(" abc,def ,, ghi ")).toEqual(["abc", "def", "ghi"]);
        expect(parsePubkeyAllowlist([" abc ", "def", ""])).toEqual(["abc", "def"]);
        expect(isPubkeyAllowed("abc", " abc,def ")).toBe(true);
        expect(isPubkeyAllowed("abc", "abcdef")).toBe(false);
    });

    test("matches case-insensitively to mirror the API allowlist", () => {
        expect(isPubkeyAllowed("ABCDEF", "abcdef")).toBe(true);
    });

    test("loads browser allowlist from public API config", async () => {
        const fetcher = async () =>
            new Response(JSON.stringify({ allowed_pubkeys: [" abc ", "def"] }), {
                status: 200,
            });

        await expect(fetchPubkeyAllowlist(fetcher)).resolves.toEqual(["abc", "def"]);
    });

    test("fails closed when public API config cannot be loaded", async () => {
        const fetcher = async () => new Response("nope", { status: 503 });

        await expect(fetchPubkeyAllowlist(fetcher)).rejects.toThrow("HTTP 503");
    });
});
