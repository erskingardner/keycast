import type { NostrEvent, ProfileContent } from "applesauce-core/helpers";
import { verifyEvent } from "nostr-tools/pure";
import { safeRemoteImageUrl } from "./image_url";

export const PROFILE_CACHE_KEY = "keycast:public-profiles:v1";
export const PROFILE_TTL_MS = 6 * 60 * 60 * 1000;
export const PROFILE_RETRY_MS = 5 * 60 * 1000;
const MAX_ENTRIES = 256;
const MAX_AGE_MS = 30 * 24 * 60 * 60 * 1000;
type Entry = {
    profile: ProfileContent | null;
    checkedAt: number;
    expiresAt: number;
};
type Storage = Pick<globalThis.Storage, "getItem" | "setItem">;

/** Cache only the public fields needed for identity display. Never trust profile HTML or URLs. */
export function displayProfile(value: unknown): ProfileContent | null {
    if (!value || typeof value !== "object" || Array.isArray(value))
        return null;
    const raw = value as Record<string, unknown>;
    const text = (...values: unknown[]) =>
        values.find((v) => typeof v === "string" && v.trim()) as
            | string
            | undefined;
    const name = text(raw.display_name, raw.displayName, raw.name)
        ?.trim()
        .slice(0, 160);
    const picture = safeRemoteImageUrl(
        text(raw.picture, raw.image)?.slice(0, 2048),
    );
    return {
        ...(name ? { display_name: name } : {}),
        ...(picture ? { picture } : {}),
    };
}

/** Relay metadata is untrusted: verify the author/signature before using any display fields. */
export function profileFromEvents(
    pubkey: string,
    events: NostrEvent[],
): ProfileContent | null {
    const candidates = events
        .filter((event) => {
            try {
                return (
                    event.kind === 0 &&
                    event.pubkey === pubkey &&
                    event.created_at <= Math.floor(Date.now() / 1000) + 60 &&
                    event.content.length <= 64 * 1024 &&
                    verifyEvent(event)
                );
            } catch {
                return false;
            }
        })
        .sort(
            (a, b) => b.created_at - a.created_at || a.id.localeCompare(b.id),
        );
    for (const event of candidates) {
        try {
            const profile = displayProfile(JSON.parse(event.content));
            if (profile) return profile;
        } catch {
            /* Ignore malformed metadata. */
        }
    }
    return null;
}

export class ProfileCache {
    private entries = new Map<string, Entry>();
    private pending = new Map<string, Promise<ProfileContent | null>>();
    private hydrated = false;

    constructor(
        private fetchProfile: (
            pubkey: string,
        ) => Promise<ProfileContent | null>,
        private storage: () => Storage | undefined = () => undefined,
        private now: () => number = Date.now,
    ) {}

    private hydrate() {
        if (this.hydrated) return;
        // SSR must not prevent hydration once this cache is used in a browser.
        try {
            const storage = this.storage();
            if (!storage) return;
            this.hydrated = true;
            const raw = storage.getItem(PROFILE_CACHE_KEY);
            if (!raw || raw.length > 1024 * 1024) return;
            const rows: unknown = JSON.parse(raw);
            if (!Array.isArray(rows)) return;
            for (const row of rows.slice(-MAX_ENTRIES)) {
                if (!Array.isArray(row) || row.length !== 2) continue;
                const [pubkey, entry] = row;
                if (
                    typeof pubkey !== "string" ||
                    !/^[a-f0-9]{64}$/.test(pubkey) ||
                    !entry ||
                    !Number.isFinite(entry.checkedAt) ||
                    !Number.isFinite(entry.expiresAt) ||
                    entry.checkedAt > this.now() ||
                    entry.checkedAt < this.now() - MAX_AGE_MS ||
                    entry.expiresAt > entry.checkedAt + PROFILE_TTL_MS
                )
                    continue;
                this.entries.set(pubkey, {
                    checkedAt: entry.checkedAt,
                    expiresAt: entry.expiresAt,
                    profile: displayProfile(entry.profile),
                });
            }
        } catch {
            /* Restricted or corrupt browser storage must never prevent rendering. */
        }
    }

    peek(pubkey: string): ProfileContent | null {
        this.hydrate();
        return this.entries.get(pubkey)?.profile ?? null;
    }

    load(pubkey: string): Promise<ProfileContent | null> {
        this.hydrate();
        if (!/^[a-f0-9]{64}$/.test(pubkey)) return Promise.resolve(null);
        const cached = this.entries.get(pubkey);
        if (cached && cached.expiresAt > this.now())
            return Promise.resolve(cached.profile);
        const pending = this.pending.get(pubkey);
        if (pending) return pending;
        const request = Promise.resolve()
            .then(() => this.fetchProfile(pubkey))
            .catch(() => null)
            .then((result) => {
                const profile =
                    displayProfile(result) ?? cached?.profile ?? null;
                const checkedAt = this.now();
                this.entries.delete(pubkey);
                this.entries.set(pubkey, {
                    profile,
                    checkedAt,
                    expiresAt:
                        checkedAt +
                        (result ? PROFILE_TTL_MS : PROFILE_RETRY_MS),
                });
                while (this.entries.size > MAX_ENTRIES)
                    this.entries.delete(this.entries.keys().next().value!);
                try {
                    this.storage()?.setItem(
                        PROFILE_CACHE_KEY,
                        JSON.stringify([...this.entries]),
                    );
                } catch {
                    /* Memory caching still works. */
                }
                return profile;
            })
            .finally(() => this.pending.delete(pubkey));
        this.pending.set(pubkey, request);
        return request;
    }
}
