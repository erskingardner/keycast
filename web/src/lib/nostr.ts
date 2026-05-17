import { EventStore } from "applesauce-core";
import type {
    EventTemplate,
    NostrEvent,
    ProfileContent,
} from "applesauce-core/helpers";
import { normalizeToPubkey, npubEncode } from "applesauce-core/helpers";
import { getEventHash, verifyEvent } from "applesauce-core/helpers/event";
import { createEventLoaderForStore } from "applesauce-loaders/loaders";
import { RelayPool } from "applesauce-relay";
import type { ISigner } from "applesauce-signers";
import {
    AmberClipboardSigner,
    ExtensionSigner,
    NostrConnectSigner,
} from "applesauce-signers";
import { catchError, filter, firstValueFrom, of, timeout } from "rxjs";
import {
    DEFAULT_NOSTR_READ_RELAYS,
    DEFAULT_OUTBOX_RELAYS,
    REQUIRED_PUBLIC_RELAYS,
} from "$lib/utils/relays";
import { NIP_98_HTTP_AUTH_KIND } from "./utils/http_auth";

export type NostrUser = {
    pubkey: string;
    npub: string;
};

export type NostrProfile = ProfileContent;
export type SignerKind = "extension" | "nostr-connect" | "amber";
export type ActiveSignerSummary = {
    kind: SignerKind;
    pubkey: string;
};
export type ActiveSigner = ActiveSignerSummary & {
    signer: ISigner;
};
export type NostrConnectSigninSession = {
    uri: string;
    waitForUser: (abort?: AbortSignal) => Promise<NostrUser>;
    cancel: () => void;
};

const PROFILE_LOAD_TIMEOUT_MS = 5000;
const CONTACTS_LOAD_TIMEOUT_MS = 5000;
const HEX_SIGNATURE = /^[0-9a-f]{128}$/i;
export const DEFAULT_NOSTR_CONNECT_RELAYS = [
    "wss://relay.nsec.app",
    ...REQUIRED_PUBLIC_RELAYS,
] as const;

export const eventStore = new EventStore();
export const relayPool = new RelayPool();
let activeSigner: ActiveSigner | null = null;

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

export function isAmberSigninSupported(): boolean {
    return (
        typeof navigator !== "undefined" &&
        /Android/i.test(navigator.userAgent)
    );
}

export function normalizeBunkerUri(uri: string): string {
    const normalized = uri.trim();
    if (!normalized.toLowerCase().startsWith("bunker://")) {
        throw new Error("Paste a bunker:// remote signer connection string");
    }

    return normalized;
}

export function buildNip46SigningPermissions(): string[] {
    return NostrConnectSigner.buildSigningPermissions([NIP_98_HTTP_AUTH_KIND]);
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

export async function getExtensionUser(): Promise<NostrUser> {
    return userFromSigner(getExtensionSigner(), "extension");
}

export async function getAmberUser(): Promise<NostrUser> {
    return userFromSigner(new ManualAmberSigner(), "amber");
}

export async function connectNostrConnectBunker(
    bunkerUri: string,
): Promise<NostrUser> {
    const signer = await NostrConnectSigner.fromBunkerURI(normalizeBunkerUri(bunkerUri), {
        pool: relayPool,
        permissions: buildNip46SigningPermissions(),
        onAuth: openSignerAuthChallenge,
    });

    return userFromSigner(signer, "nostr-connect");
}

export function createNostrConnectSigninSession(
    relays: readonly string[] = DEFAULT_NOSTR_CONNECT_RELAYS,
): NostrConnectSigninSession {
    const signer = new NostrConnectSigner({
        relays: [...relays],
        pool: relayPool,
        onAuth: openSignerAuthChallenge,
    });
    const uri = signer.getNostrConnectURI({
        name: "Keycast",
        url: browserOrigin(),
        permissions: buildNip46SigningPermissions(),
    });

    return {
        uri,
        waitForUser: async (abort?: AbortSignal) => {
            await signer.waitForSigner(abort);
            return userFromSigner(signer, "nostr-connect");
        },
        cancel: () => {
            void signer.close();
        },
    };
}

export function setActiveSigner(next: ActiveSigner): ActiveSigner {
    const pubkey = normalizePubkey(next.pubkey);
    if (!pubkey) {
        throw new Error("Signer returned an invalid pubkey");
    }

    if (activeSigner?.signer !== next.signer) {
        disposeSigner(activeSigner?.signer);
    }

    activeSigner = {
        kind: next.kind,
        pubkey,
        signer: next.signer,
    };
    return activeSigner;
}

export function clearActiveSigner(): void {
    disposeSigner(activeSigner?.signer);
    activeSigner = null;
}

export function getActiveSignerSummary(): ActiveSignerSummary | null {
    if (!activeSigner) return null;

    return {
        kind: activeSigner.kind,
        pubkey: activeSigner.pubkey,
    };
}

export async function signNostrEvent(
    template: EventTemplate,
    expectedPubkey?: string,
): Promise<NostrEvent> {
    const signer = activeSigner?.signer ?? getExtensionSigner();
    const normalizedExpectedPubkey = expectedPubkey
        ? normalizePubkey(expectedPubkey)
        : null;

    if (
        activeSigner &&
        normalizedExpectedPubkey &&
        activeSigner.pubkey !== normalizedExpectedPubkey
    ) {
        throw new Error("The active signer is connected to a different pubkey");
    }

    const signedEvent = await signer.signEvent(template);

    if (
        normalizedExpectedPubkey &&
        normalizePubkey(signedEvent.pubkey) !== normalizedExpectedPubkey
    ) {
        throw new Error("The signer signed with a different pubkey");
    }

    return signedEvent;
}

async function userFromSigner(signer: ISigner, kind: SignerKind): Promise<NostrUser> {
    const pubkey = normalizePubkey(await signer.getPublicKey());
    if (!pubkey) {
        throw new Error("The signer did not return a valid pubkey");
    }

    const user = userFromPubkey(pubkey);
    if (!user) {
        throw new Error("The signer did not return a valid pubkey");
    }

    setActiveSigner({ kind, signer, pubkey });
    return user;
}

class ManualAmberSigner implements ISigner {
    pubkey?: string;

    async getPublicKey(): Promise<string> {
        if (!isAmberSigninSupported()) {
            throw new Error("Amber signing is only available on Android");
        }

        if (this.pubkey) return this.pubkey;

        const result = await requestAmberResult(
            AmberClipboardSigner.createGetPublicKeyIntent(),
            "public key",
            (value) => normalizePubkey(value) !== null,
        );
        const pubkey = normalizePubkey(result);
        if (!pubkey) throw new Error("Expected Amber to return a pubkey");

        this.pubkey = pubkey;
        return pubkey;
    }

    async signEvent(
        template: EventTemplate & { pubkey?: string },
    ): Promise<NostrEvent> {
        if (!isAmberSigninSupported()) {
            throw new Error("Amber signing is only available on Android");
        }

        const signerPubkey = template.pubkey ?? this.pubkey;
        const pubkey = signerPubkey ? normalizePubkey(signerPubkey) : null;
        if (!pubkey) throw new Error("Unknown Amber signer pubkey");

        const draftWithPubkey = { ...template, pubkey };
        const draftWithId = {
            ...draftWithPubkey,
            id: getEventHash(draftWithPubkey),
        };
        const result = await requestAmberResult(
            AmberClipboardSigner.createSignEventIntent(
                draftWithId as EventTemplate,
            ),
            "signature",
            (value) => HEX_SIGNATURE.test(value),
        );
        const signature = result.trim();
        const event = { ...draftWithId, sig: signature };
        if (!verifyEvent(event)) {
            throw new Error("Amber returned an invalid signature");
        }

        return event;
    }
}

async function requestAmberResult(
    intent: string,
    label: string,
    accepts: (value: string) => boolean,
): Promise<string> {
    if (typeof window === "undefined" || typeof document === "undefined") {
        throw new Error("Amber signing requires a browser");
    }

    window.open(intent, "_blank");
    const returnedFromAmber = await waitForBrowserToReturn();

    if (returnedFromAmber) {
        const clipboardResult = (await readClipboardText()).trim();
        if (clipboardResult && accepts(clipboardResult)) {
            return clipboardResult;
        }
    }

    const manualResult = window.prompt(`Paste the Amber ${label} result`);
    const trimmedManualResult = manualResult?.trim() ?? "";
    if (trimmedManualResult && accepts(trimmedManualResult)) {
        return trimmedManualResult;
    }

    throw new Error(`Amber ${label} result was invalid or not provided`);
}

async function waitForBrowserToReturn(): Promise<boolean> {
    return new Promise<boolean>((resolve) => {
        let sawHidden = document.visibilityState === "hidden";
        let settled = false;

        const done = (returnedFromAmber: boolean) => {
            if (settled) return;
            settled = true;
            document.removeEventListener("visibilitychange", onVisibilityChange);
            window.removeEventListener("focus", onFocus);
            resolve(returnedFromAmber);
        };

        const onVisibilityChange = () => {
            if (document.visibilityState === "hidden") {
                sawHidden = true;
            } else if (sawHidden) {
                setTimeout(() => done(true), 250);
            }
        };

        const onFocus = () => {
            if (sawHidden) {
                setTimeout(() => done(true), 250);
            }
        };

        document.addEventListener("visibilitychange", onVisibilityChange);
        window.addEventListener("focus", onFocus);
        setTimeout(() => {
            if (!sawHidden && document.visibilityState === "visible") {
                done(false);
            }
        }, 1500);
    });
}

async function readClipboardText(): Promise<string> {
    try {
        if (navigator.clipboard?.readText) {
            return await navigator.clipboard.readText();
        }
    } catch {
        // Android browsers often require a user gesture before granting clipboard access.
    }

    return "";
}

function disposeSigner(signer: ISigner | null | undefined): void {
    const disposable = signer as
        | {
              close?: () => Promise<void> | void;
              destroy?: () => void;
          }
        | null
        | undefined;

    if (disposable?.close) void disposable.close();
    disposable?.destroy?.();
}

function openSignerAuthChallenge(url: string): Promise<void> {
    if (typeof window !== "undefined") {
        window.open(
            url,
            "keycast-signer-auth",
            "width=420,height=640,resizable=yes,status=no,location=yes,toolbar=no,menubar=no",
        );
    }

    return Promise.resolve();
}

function browserOrigin(): string | undefined {
    return typeof location === "undefined" ? undefined : location.origin;
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
