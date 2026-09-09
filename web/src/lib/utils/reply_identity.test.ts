import { beforeEach, describe, expect, test } from "bun:test";
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

/** A store that throws on every access, like a browser with site data blocked. */
function throwingStorage(): Storage {
    const boom = () => {
        throw new Error("storage is disabled");
    };
    return {
        get length(): number {
            return boom();
        },
        clear: boom,
        getItem: boom,
        key: boom,
        removeItem: boom,
        setItem: boom,
    } as unknown as Storage;
}

/** Readable until revoked, like storage that stops working mid-session. */
function revocableStorage(): Storage & { revoke: () => void } {
    const inner = memoryStorage();
    let live = true;
    const guard = <T>(operation: () => T): T => {
        if (!live) throw new Error("storage is disabled");
        return operation();
    };
    return {
        get length(): number {
            return guard(() => inner.length);
        },
        clear: () => guard(() => inner.clear()),
        getItem: (key: string) => guard(() => inner.getItem(key)),
        key: (index: number) => guard(() => inner.key(index)),
        removeItem: (key: string) => guard(() => inner.removeItem(key)),
        setItem: (key: string, value: string) =>
            guard(() => inner.setItem(key, value)),
        revoke: () => {
            live = false;
        },
    } as Storage & { revoke: () => void };
}

const PIN_KEY = "keycast:management-reply-identity:v1";
const first = "a".repeat(64);
const second = "b".repeat(64);

describe("management reply identity pinning", () => {
    beforeEach(() => forgetManagementReplyKey(memoryStorage()));

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

    test("retains the pin in memory when the store is missing", () => {
        // A first use still establishes a trust anchor for the rest of the session.
        expect(verifyManagementReplyKey(first, undefined)).toBe(first);
        expect(pinnedManagementReplyKey(undefined)).toBe(first);
        expect(() => verifyManagementReplyKey(second, undefined)).toThrow(
            "identity changed",
        );
        expect(() => forgetManagementReplyKey(undefined)).not.toThrow();
        expect(pinnedManagementReplyKey(undefined)).toBeNull();
    });

    test("retains the pin in memory when every storage access throws", () => {
        const store = throwingStorage();
        expect(verifyManagementReplyKey(first, store)).toBe(first);
        expect(pinnedManagementReplyKey(store)).toBe(first);
        // Without the session pin this second, different key would be accepted
        // as another first use and a compromised API could swap the identity.
        expect(() => verifyManagementReplyKey(second, store)).toThrow(
            "identity changed",
        );
        expect(() => trustManagementReplyKey(second, store)).not.toThrow();
        expect(verifyManagementReplyKey(second, store)).toBe(second);
    });

    test("a persisted pin outranks a stale session pin", () => {
        const store = memoryStorage();
        trustManagementReplyKey(second, undefined);
        store.setItem(PIN_KEY, first);
        expect(pinnedManagementReplyKey(store)).toBe(first);
    });

    test("a pin read from storage survives storage becoming unreadable", () => {
        // The returning-browser case: a persisted pin and a fresh session, then
        // storage stops working. Without mirroring the read into memory this
        // would look like a first use and accept a substituted identity.
        const store = revocableStorage();
        store.setItem(PIN_KEY, first);
        expect(verifyManagementReplyKey(first, store)).toBe(first);
        store.revoke();
        expect(() => verifyManagementReplyKey(second, store)).toThrow(
            "identity changed",
        );
        expect(verifyManagementReplyKey(first, store)).toBe(first);
    });

    test("fingerprints are short, stable and never the whole key", () => {
        expect(managementReplyFingerprint(first)).toBe("aaaaaaaa…aaaaaaaa");
        expect(managementReplyFingerprint(first).length).toBeLessThan(
            first.length,
        );
        expect(managementReplyFingerprint("nope")).toBe("unknown");
    });
});
