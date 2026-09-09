import { userFromPubkey, type NostrUser } from "$lib/nostr";

let currentUser: CurrentUser | null = $state(null);

class CurrentUser {
    /** The Nostr user currently signed in through a browser, remote, or Android signer. */
    user: NostrUser | null = $state(null);

    constructor(pubkey: string) {
        this.user = userFromPubkey(pubkey);
    }
}

export function getCurrentUser(): CurrentUser | null {
    return currentUser;
}

export function setCurrentUser(pubkey: string | null): CurrentUser | null {
    currentUser = pubkey ? new CurrentUser(pubkey) : null;
    return currentUser;
}
