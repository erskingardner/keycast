import { describe, expect, test } from "bun:test";
import {
    forgetManagementReplyKey,
    isManagementReplyKey,
    managementReplyFingerprint,
    pinnedManagementReplyKey,
    trustManagementReplyKey,
    verifyManagementReplyKey,
} from "./reply_identity";

/** Minimal Storage stand-in; Bun has no localStorage. */
function memoryStorage(): Storage {
    const values = new Map<string, string>();
    return {
        get length() {
            return values.size;
        },
        clear: () => values.clear(),
        getItem: (key: string) => values.get(key) ?? null,
        key: (index: number) => [...values.keys()][index] ?? null,
        removeItem: (key: string) => void values.delete(key),
        setItem: (key: string, value: string) => void values.set(key, value),
    } as Storage;
}

const first = "a".repeat(64);
const second = "b".repeat(64);

describe("management reply identity pinning", () => {
    test("pins on first use and accepts the same identity afterwards", () => {
        const store = memoryStorage();
        expect(pinnedManagementReplyKey(store)).toBeNull();
        expect(verifyManagementReplyKey(first, store)).toBe(first);
        expect(pinnedManagementReplyKey(store)).toBe(first);
        expect(verifyManagementReplyKey(first, store)).toBe(first);
    });

    test("fails closed when the identity changes until it is trusted", () => {
        const store = memoryStorage();
        verifyManagementReplyKey(first, store);
        expect(() => verifyManagementReplyKey(second, store)).toThrow(
            "identity changed",
        );
        // The message must let the operator compare against the host.
        expect(() => verifyManagementReplyKey(second, store)).toThrow(
            "keycast_signer status",
        );
        trustManagementReplyKey(second, store);
        expect(verifyManagementReplyKey(second, store)).toBe(second);
        expect(() => verifyManagementReplyKey(first, store)).toThrow(
            "identity changed",
        );
    });

    test("rejects anything that is not a 32-byte lowercase hex key", () => {
        const store = memoryStorage();
        for (const invalid of [
            undefined,
            null,
            "",
            "zz",
            "A".repeat(64),
            "a".repeat(63),
            "a".repeat(65),
            123,
            { pubkey: first },
        ]) {
            expect(() => verifyManagementReplyKey(invalid, store)).toThrow(
                "did not publish a valid management reply identity",
            );
            expect(isManagementReplyKey(invalid)).toBe(false);
        }
        expect(pinnedManagementReplyKey(store)).toBeNull();
        expect(() => trustManagementReplyKey("nope", store)).toThrow("Invalid");
    });

    test("forgetting the pin allows a fresh first use", () => {
        const store = memoryStorage();
        verifyManagementReplyKey(first, store);
        forgetManagementReplyKey(store);
        expect(pinnedManagementReplyKey(store)).toBeNull();
        expect(verifyManagementReplyKey(second, store)).toBe(second);
    });

    test("works without any storage and never throws on a missing store", () => {
        expect(verifyManagementReplyKey(first, undefined)).toBe(first);
        expect(pinnedManagementReplyKey(undefined)).toBeNull();
        expect(() => forgetManagementReplyKey(undefined)).not.toThrow();
        expect(() => trustManagementReplyKey(first, undefined)).not.toThrow();
    });

    test("fingerprints are short, stable and never the whole key", () => {
        expect(managementReplyFingerprint(first)).toBe("aaaaaaaa…aaaaaaaa");
        expect(managementReplyFingerprint(first).length).toBeLessThan(
            first.length,
        );
        expect(managementReplyFingerprint("nope")).toBe("unknown");
    });
});
