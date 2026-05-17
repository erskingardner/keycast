import { EventStore } from "applesauce-core";
import type {
    EventTemplate,
    NostrEvent,
    ProfileContent,
} from "applesauce-core/helpers";
import {
    bytesToHex,
    getEventHash,
    hexToBytes,
    verifyEvent,
} from "applesauce-core/helpers/event";
import { normalizeToPubkey, npubEncode } from "applesauce-core/helpers";
import { createEventLoaderForStore } from "applesauce-loaders/loaders";
import { RelayPool } from "applesauce-relay";
import {
    AmberClipboardSigner,
    ExtensionSigner,
    NostrConnectSigner,
    PrivateKeySigner,
    type ISigner,
} from "applesauce-signers";
import { catchError, filter, firstValueFrom, of, timeout } from "rxjs";
import {
    parseStoredSignerSession,
    serializeStoredSignerSession,
    type StoredSignerSession,
} from "$lib/utils/signer_session";
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
const REMOTE_SIGNER_TIMEOUT_MS = 30000;
const NIP_98_HTTP_AUTH_KIND = 27235;
const SIGNER_SESSION_STORAGE_KEY = "keycastSignerSession";
const HEX_SIGNATURE = /^[0-9a-f]{128}$/i;

export const eventStore = new EventStore();
export const relayPool = new RelayPool();
NostrConnectSigner.pool = relayPool;

let activeSigner: ISigner | null = null;
let activeSignerSession: StoredSignerSession | null = null;

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
    const signer = getExtensionSigner();
    const pubkey = normalizePubkey(await signer.getPublicKey());
    if (!pubkey) {
        throw new Error("The NIP-07 extension did not return a valid pubkey");
    }

    rememberSigner({ kind: "extension", pubkey }, signer);
    return pubkey;
}

export function hasAmberSignerSupport(): boolean {
    return (
        typeof navigator !== "undefined" &&
        /Android/i.test(navigator.userAgent)
    );
}

export async function getAmberPubkey(): Promise<string> {
    const signer = new ManualAmberSigner();
    const pubkey = normalizePubkey(await signer.getPublicKey());
    if (!pubkey) {
        throw new Error("Amber did not return a valid pubkey");
    }

    rememberSigner({ kind: "amber", pubkey }, signer);
    return pubkey;
}

export async function getRemoteSignerPubkey(bunkerUri: string): Promise<string> {
    const { signer, session } = await connectRemoteSigner(bunkerUri);
    rememberSigner(session, signer);
    return session.pubkey;
}

export function clearSignerSession() {
    activeSigner = null;
    activeSignerSession = null;
    browserStorage()?.removeItem(SIGNER_SESSION_STORAGE_KEY);
}

export async function signNostrEvent(
    template: EventTemplate,
    expectedPubkey?: string,
): Promise<NostrEvent> {
    const signer = await signerForRequest(expectedPubkey);
    const signedEvent =
        signer instanceof ManualAmberSigner && expectedPubkey
            ? await signer.signEvent({
                  ...template,
                  pubkey: expectedPubkey,
              } as EventTemplate & { pubkey: string })
            : await signer.signEvent(template);
    const normalizedExpectedPubkey = expectedPubkey
        ? normalizePubkey(expectedPubkey)
        : null;

    if (
        normalizedExpectedPubkey &&
        normalizePubkey(signedEvent.pubkey) !== normalizedExpectedPubkey
    ) {
        throw new Error("The signer signed with a different pubkey");
    }

    return signedEvent;
}

function browserStorage(): Storage | null {
    if (typeof window === "undefined") return null;
    return window.localStorage;
}

function readStoredSignerSession(): StoredSignerSession | null {
    return parseStoredSignerSession(
        browserStorage()?.getItem(SIGNER_SESSION_STORAGE_KEY),
    );
}

function rememberSigner(session: StoredSignerSession, signer: ISigner) {
    activeSignerSession = session;
    activeSigner = signer;
    browserStorage()?.setItem(
        SIGNER_SESSION_STORAGE_KEY,
        serializeStoredSignerSession(session),
    );
}

async function signerForRequest(expectedPubkey?: string): Promise<ISigner> {
    const normalizedExpectedPubkey = expectedPubkey
        ? normalizePubkey(expectedPubkey)
        : null;
    const storedSession = readStoredSignerSession();

    if (
        activeSigner &&
        activeSignerSession &&
        (!normalizedExpectedPubkey ||
            activeSignerSession.pubkey === normalizedExpectedPubkey)
    ) {
        return activeSigner;
    }

    if (storedSession) {
        if (
            normalizedExpectedPubkey &&
            storedSession.pubkey !== normalizedExpectedPubkey
        ) {
            throw new Error("Stored signer session does not match the signed-in user");
        }

        const signer = await signerFromSession(storedSession);
        activeSigner = signer;
        activeSignerSession = storedSession;
        return signer;
    }

    return getExtensionSigner();
}

async function signerFromSession(session: StoredSignerSession): Promise<ISigner> {
    if (session.kind === "extension") {
        return getExtensionSigner();
    }

    if (session.kind === "amber") {
        return new ManualAmberSigner(session.pubkey);
    }

    return (await connectRemoteSigner(
        session.bunkerUri,
        session.clientSecretHex,
    )).signer;
}

async function connectRemoteSigner(
    bunkerUri: string,
    clientSecretHex?: string,
): Promise<{ signer: NostrConnectSigner; session: StoredSignerSession }> {
    const parsed = NostrConnectSigner.parseBunkerURI(bunkerUri);
    const clientSigner = clientSecretHex
        ? new PrivateKeySigner(hexToBytes(clientSecretHex))
        : new PrivateKeySigner();
    const signer = new NostrConnectSigner({
        relays: parsed.relays,
        remote: parsed.remote,
        signer: clientSigner,
        pool: relayPool,
        onAuth: async (url) => {
            window.open(url, "auth", "width=400,height=600");
        },
    });

    try {
        await withTimeout(
            signer.connect(
                parsed.secret,
                NostrConnectSigner.buildSigningPermissions([
                    NIP_98_HTTP_AUTH_KIND,
                ]),
            ),
            REMOTE_SIGNER_TIMEOUT_MS,
            "Remote signer connection timed out",
        );

        const pubkey = normalizePubkey(
            await withTimeout(
                signer.getPublicKey(),
                REMOTE_SIGNER_TIMEOUT_MS,
                "Remote signer did not return a pubkey",
            ),
        );
        if (!pubkey) throw new Error("Remote signer returned an invalid pubkey");

        return {
            signer,
            session: {
                kind: "remote",
                pubkey,
                bunkerUri,
                clientSecretHex: bytesToHex(clientSigner.key),
            },
        };
    } catch (error) {
        await signer.close();
        throw error;
    }
}

async function withTimeout<T>(
    promise: Promise<T>,
    timeoutMs: number,
    message: string,
): Promise<T> {
    let timeoutId: ReturnType<typeof setTimeout> | undefined;
    const timeoutPromise = new Promise<never>((_, reject) => {
        timeoutId = setTimeout(() => reject(new Error(message)), timeoutMs);
    });

    try {
        return await Promise.race([promise, timeoutPromise]);
    } finally {
        if (timeoutId) clearTimeout(timeoutId);
    }
}

class ManualAmberSigner implements ISigner {
    constructor(public pubkey?: string) {}

    async getPublicKey(): Promise<string> {
        if (!hasAmberSignerSupport()) {
            throw new Error("Amber signing is only available on Android");
        }

        if (this.pubkey) return this.pubkey;

        const result = await requestAmberResult(
            AmberClipboardSigner.createGetPublicKeyIntent(),
            "public key",
            false,
        );
        const pubkey = normalizePubkey(result);
        if (!pubkey) throw new Error("Expected Amber to return a pubkey");

        this.pubkey = pubkey;
        return pubkey;
    }

    async signEvent(
        template: EventTemplate & { pubkey?: string },
    ): Promise<NostrEvent> {
        if (!hasAmberSignerSupport()) {
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
            AmberClipboardSigner.createSignEventIntent(draftWithId),
            "signature",
        );
        const signature = result.trim();
        if (!HEX_SIGNATURE.test(signature)) {
            throw new Error("Expected Amber to return a hex signature");
        }

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
    readClipboardOnReturn = true,
): Promise<string> {
    if (typeof window === "undefined" || typeof document === "undefined") {
        throw new Error("Amber signing requires a browser");
    }

    window.open(intent, "_blank");
    const returnedFromAmber = await waitForBrowserToReturn();

    if (returnedFromAmber && readClipboardOnReturn) {
        const clipboardResult = await readClipboardText();
        if (clipboardResult.trim()) return clipboardResult.trim();
    }

    const manualResult = window.prompt(`Paste the Amber ${label} result`);
    if (manualResult?.trim()) return manualResult.trim();

    throw new Error(`Amber ${label} result was not provided`);
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
