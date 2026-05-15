import {
    npubForPubkey,
    userFromPubkey,
    type NostrUser,
} from "$lib/nostr";

export function truncatedNpubForPubkey(pubkey?: string, maxLength = 9) {
    return npubForPubkey(pubkey)?.slice(0, maxLength);
}

export function userFromPubkeyOrNpub(pubkeyOrNpub: string): NostrUser | null {
    return userFromPubkey(pubkeyOrNpub);
}
