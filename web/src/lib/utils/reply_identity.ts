/**
 * Pins the signer's management reply identity.
 *
 * Management write replies are NIP-44 encrypted to a per-request browser key.
 * NIP-44 authenticates the two parties of the exchange, so it only proves who
 * sent a reply if this side chooses the sender key instead of reading it out of
 * the response. The signer derives a stable identity from its root credential
 * and publishes the public half through `/config`; pinning it here means a
 * compromised API cannot substitute its own key and fabricate an outcome, such
 * as a `bunker://` URI pointing at an attacker's signer.
 *
 * The pin is per browser. It is a convenience anchor, not the root of trust: the
 * authoritative check is comparing the fingerprint against
 * `keycast_signer status` on the host, which the Status page shows.
 */
const PIN_STORAGE_KEY = "keycast:management-reply-identity:v1";

/** Storage access throws in some privacy modes; treat that as "nothing pinned". */
function browserStorage(): Storage | undefined {
    try {
        return typeof localStorage === "undefined" ? undefined : localStorage;
    } catch {
        return undefined;
    }
}

export function isManagementReplyKey(value: unknown): value is string {
    return typeof value === "string" && /^[0-9a-f]{64}$/.test(value);
}

export function pinnedManagementReplyKey(
    store: Storage | undefined = browserStorage(),
): string | null {
    try {
        return store?.getItem(PIN_STORAGE_KEY) ?? null;
    } catch {
        return null;
    }
}

export function trustManagementReplyKey(
    publicKey: string,
    store: Storage | undefined = browserStorage(),
): void {
    if (!isManagementReplyKey(publicKey)) {
        throw new Error("Invalid management reply identity");
    }
    try {
        store?.setItem(PIN_STORAGE_KEY, publicKey);
    } catch {
        // Without storage the identity is verified per session instead of pinned.
    }
}

export function forgetManagementReplyKey(
    store: Storage | undefined = browserStorage(),
): void {
    try {
        store?.removeItem(PIN_STORAGE_KEY);
    } catch {
        // Nothing was pinned.
    }
}

/** Short form for display and for comparison against the host CLI. */
export function managementReplyFingerprint(publicKey: string): string {
    if (!isManagementReplyKey(publicKey)) return "unknown";
    return `${publicKey.slice(0, 8)}…${publicKey.slice(-8)}`;
}

/**
 * Returns the identity to encrypt to, pinning it on first use. Fails closed when
 * a different identity appears, so the change has to be confirmed deliberately.
 */
export function verifyManagementReplyKey(
    published: unknown,
    store: Storage | undefined = browserStorage(),
): string {
    if (!isManagementReplyKey(published)) {
        throw new Error(
            "The signer did not publish a valid management reply identity",
        );
    }
    const pinned = pinnedManagementReplyKey(store);
    if (!pinned) {
        trustManagementReplyKey(published, store);
        return published;
    }
    if (pinned !== published) {
        throw new Error(
            `This instance's management reply identity changed (pinned ${managementReplyFingerprint(pinned)}, received ${managementReplyFingerprint(published)}). ` +
                'Compare it with "keycast_signer status" on your host, then trust the new identity on the Status page if you rotated the root credential.',
        );
    }
    return published;
}
