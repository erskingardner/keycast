import { EventStore } from "applesauce-core";
import type {
    EventTemplate,
    NostrEvent,
    ProfileContent,
} from "applesauce-core/helpers";
import { normalizeToPubkey, npubEncode } from "applesauce-core/helpers";
import { createEventLoaderForStore } from "applesauce-loaders/loaders";
import { RelayPool } from "applesauce-relay";
import { ExtensionSigner } from "applesauce-signers";
import { catchError, filter, firstValueFrom, of, timeout } from "rxjs";
import {
    DEFAULT_NOSTR_READ_RELAYS,
    DEFAULT_OUTBOX_RELAYS,
} from "$lib/utils/relays";

export type NostrUser = {
    pubkey: string;
    npub: string;
};

export type NostrProfile = ProfileContent;

const PROFILE_LOAD_TIMEOUT_MS = 5000;
const CONTACTS_LOAD_TIMEOUT_MS = 5000;

export const eventStore = new EventStore();
export const relayPool = new RelayPool();

const loaderRelays = Array.from(
    new Set([...DEFAULT_NOSTR_READ_RELAYS, ...DEFAULT_OUTBOX_RELAYS]),
);

createEventLoaderForStore(eventStore, relayPool, {
    bufferTime: 1000,
    followRelayHints: true,
    extraRelays: [...DEFAULT_NOSTR_READ_RELAYS],
    lookupRelays: loaderRelays,
});

const profileCache = new Map<string, Promise<NostrProfile | null>>();

export function normalizePubkey(pubkeyOrNpub: string): string | null {
    const input = pubkeyOrNpub.trim();
    if (!input) return null;

    try {
        return normalizeToPubkey(input);
    } catch {
        return null;
    }
}

export function userFromPubkey(pubkey: string | null | undefined): NostrUser | null {
    const normalized = pubkey ? normalizePubkey(pubkey) : null;
    if (!normalized) return null;

    return {
        pubkey: normalized,
        npub: npubEncode(normalized),
    };
}

export function npubForPubkey(pubkey: string | null | undefined): string | undefined {
    return userFromPubkey(pubkey)?.npub;
}

export function hasNip07Extension(): boolean {
    return typeof window !== "undefined" && !!window.nostr;
}

export function getExtensionSigner(): ExtensionSigner {
    if (!hasNip07Extension()) {
        throw new Error("Install or enable a NIP-07 browser extension to sign in");
    }

    return new ExtensionSigner();
}

export async function getExtensionPubkey(): Promise<string> {
    const pubkey = normalizePubkey(await getExtensionSigner().getPublicKey());
    if (!pubkey) {
        throw new Error("The NIP-07 extension did not return a valid pubkey");
    }

    return pubkey;
}

export async function signNostrEvent(
    template: EventTemplate,
    expectedPubkey?: string,
): Promise<NostrEvent> {
    const signer = getExtensionSigner();
    const signedEvent = await signer.signEvent(template);
    const normalizedExpectedPubkey = expectedPubkey
        ? normalizePubkey(expectedPubkey)
        : null;

    if (
        normalizedExpectedPubkey &&
        normalizePubkey(signedEvent.pubkey) !== normalizedExpectedPubkey
    ) {
        throw new Error("The NIP-07 extension signed with a different pubkey");
    }

    return signedEvent;
}

export async function loadProfile(
    pubkey: string | null | undefined,
): Promise<NostrProfile | null> {
    const normalized = pubkey ? normalizePubkey(pubkey) : null;
    if (!normalized) return null;

    const cached = profileCache.get(normalized);
    if (cached) return cached;

    const promise = firstValueFrom(
        eventStore.profile(normalized).pipe(
            filter((profile): profile is NostrProfile => !!profile),
            timeout({ first: PROFILE_LOAD_TIMEOUT_MS }),
            catchError(() => of(null)),
        ),
    ).then((profile) => {
        if (!profile) {
            profileCache.delete(normalized);
        }

        return profile;
    });

    profileCache.set(normalized, promise);
    return promise;
}

export async function loadFollowPubkeys(
    pubkey: string | null | undefined,
): Promise<string[]> {
    const normalized = pubkey ? normalizePubkey(pubkey) : null;
    if (!normalized) return [];

    const contacts = await firstValueFrom(
        eventStore.contacts(normalized).pipe(
            timeout({ first: CONTACTS_LOAD_TIMEOUT_MS }),
            catchError(() => of([])),
        ),
    );

    return contacts
        .map((contact) => normalizePubkey(contact.pubkey))
        .filter((contactPubkey): contactPubkey is string => !!contactPubkey);
}
