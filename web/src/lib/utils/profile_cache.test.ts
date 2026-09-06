import { describe, expect, test } from "bun:test";
import { finalizeEvent, getPublicKey } from "nostr-tools/pure";
import {
    ProfileCache,
    PROFILE_CACHE_KEY,
    PROFILE_RETRY_MS,
    PROFILE_TTL_MS,
    displayProfile,
    profileFromEvents,
} from "./profile_cache";

const secret = new Uint8Array(32).fill(1);
const pubkey = getPublicKey(secret);
const profile = {
    display_name: "Alice",
    picture: "https://example.com/avatar.png",
};
function storage() {
    const rows = new Map<string, string>();
    return {
        getItem: (key: string) => rows.get(key) ?? null,
        setItem: (key: string, value: string) => {
            rows.set(key, value);
        },
    };
}

describe("public profile cache", () => {
    test("deduplicates concurrent lookups and persists only display fields across reloads", async () => {
        const disk = storage();
        let calls = 0;
        const cache = new ProfileCache(
            async () => {
                calls++;
                return { ...profile, about: "not stored" };
            },
            () => disk,
        );
        const [a, b] = await Promise.all([
            cache.load(pubkey),
            cache.load(pubkey),
        ]);
        expect(a).toEqual(profile);
        expect(b).toEqual(profile);
        expect(calls).toBe(1);
        const reloaded = new ProfileCache(
            async () => {
                throw new Error("Should not fetch");
            },
            () => disk,
        );
        expect(reloaded.peek(pubkey)).toEqual(profile);
        expect(await reloaded.load(pubkey)).toEqual(profile);
        expect(disk.getItem(PROFILE_CACHE_KEY)).not.toContain("not stored");
    });

    test("shows stale metadata immediately, refreshes after TTL and retains it offline", async () => {
        let now = 1_000_000;
        let next: typeof profile | null = profile;
        let calls = 0;
        const cache = new ProfileCache(
            async () => {
                calls++;
                return next;
            },
            storage,
            () => now,
        );
        await cache.load(pubkey);
        now += PROFILE_TTL_MS + 1;
        expect(cache.peek(pubkey)).toEqual(profile);
        next = { ...profile, display_name: "Alice updated" };
        expect(await cache.load(pubkey)).toEqual(next);
        now += PROFILE_TTL_MS + 1;
        next = null;
        expect((await cache.load(pubkey))?.display_name).toBe("Alice updated");
        await cache.load(pubkey);
        expect(calls).toBe(3);
        now += PROFILE_RETRY_MS + 1;
        await cache.load(pubkey);
        expect(calls).toBe(4);
    });

    test("caches misses and survives storage failure and corrupt persisted data", async () => {
        let calls = 0;
        const cache = new ProfileCache(
            async () => {
                calls++;
                return null;
            },
            () => {
                throw new Error("blocked");
            },
        );
        await cache.load(pubkey);
        await cache.load(pubkey);
        expect(calls).toBe(1);
        const disk = storage();
        disk.setItem(PROFILE_CACHE_KEY, "broken JSON");
        const fresh = new ProfileCache(
            async () => profile,
            () => disk,
        );
        expect(await fresh.load(pubkey)).toEqual(profile);
    });

    test("bounds persistent cache size", async () => {
        const disk = storage();
        const cache = new ProfileCache(
            async () => profile,
            () => disk,
        );
        for (let i = 0; i < 270; i++)
            await cache.load(i.toString(16).padStart(64, "0"));
        expect(JSON.parse(disk.getItem(PROFILE_CACHE_KEY)!).length).toBe(256);
    });

    test("sanitizes metadata and ignores unexpected field types and unsafe images", () => {
        expect(
            displayProfile({
                display_name: [],
                name: " Alice ",
                picture: "javascript:alert(1)",
                about: "ignored",
            }),
        ).toEqual({ display_name: "Alice" });
        expect(displayProfile([])).toBeNull();
        expect(
            displayProfile({ display_name: "a".repeat(1000) })?.display_name
                ?.length,
        ).toBe(160);
    });

    test("uses newest valid signed metadata from the requested author", () => {
        const sign = (created_at: number, content: string) =>
            finalizeEvent({ kind: 0, created_at, tags: [], content }, secret);
        const older = sign(1, JSON.stringify({ name: "Old" }));
        const newer = sign(2, JSON.stringify(profile));
        const forged = JSON.parse(
            JSON.stringify(sign(3, JSON.stringify({ name: "Forged" }))),
        );
        forged.sig = "0".repeat(128);
        expect(profileFromEvents(pubkey, [forged, older, newer])).toEqual(
            profile,
        );
        expect(profileFromEvents("f".repeat(64), [newer])).toBeNull();
        expect(profileFromEvents(pubkey, [sign(4, "not json")])).toBeNull();
    });
});
