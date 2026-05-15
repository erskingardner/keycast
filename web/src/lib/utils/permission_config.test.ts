import { describe, expect, test } from "bun:test";
import { parseAllowedKindsInput, parseBlockedWordsInput } from "./permission_config";

describe("permission config parsing", () => {
    test("uses null for blank allowed kinds so the backend treats it as all kinds", () => {
        expect(parseAllowedKindsInput("")).toBeNull();
        expect(parseAllowedKindsInput(" , ")).toBeNull();
    });

    test("parses allowed kinds and drops invalid tokens without making the policy allow-all", () => {
        expect(parseAllowedKindsInput("1, 7, 10002")).toEqual([1, 7, 10002]);
        expect(parseAllowedKindsInput("bad, 70000, -1")).toEqual([]);
    });

    test("trims blocked words and ignores empty comma segments", () => {
        expect(parseBlockedWordsInput(" spam, scam ,, phishing ")).toEqual([
            "spam",
            "scam",
            "phishing",
        ]);
        expect(parseBlockedWordsInput(" , ")).toBeNull();
    });
});
