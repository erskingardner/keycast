import { describe, expect, test } from "bun:test";
import { isPubkeyAllowed, parsePubkeyAllowlist } from "./allowlist";

describe("pubkey allowlist helpers", () => {
    test("treats blank allowlists as open", () => {
        expect(isPubkeyAllowed("abc", "")).toBe(true);
        expect(isPubkeyAllowed("abc", " , ")).toBe(true);
        expect(isPubkeyAllowed("abc", undefined)).toBe(true);
    });

    test("trims entries and requires exact matches", () => {
        expect(parsePubkeyAllowlist(" abc,def ,, ghi ")).toEqual(["abc", "def", "ghi"]);
        expect(isPubkeyAllowed("abc", " abc,def ")).toBe(true);
        expect(isPubkeyAllowed("abc", "abcdef")).toBe(false);
    });

    test("matches case-insensitively to mirror the API allowlist", () => {
        expect(isPubkeyAllowed("ABCDEF", "abcdef")).toBe(true);
    });
});
