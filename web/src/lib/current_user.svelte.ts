import {
    loadFollowPubkeys,
    userFromPubkey,
    type NostrUser,
} from "$lib/nostr";

let currentUser: CurrentUser | null = $state(null);

class CurrentUser {
    /** The Nostr user currently signed in through NIP-07. */
    user: NostrUser | null = $state(null);

    /** Array of pubkeys that the current user follows */
    follows: string[] = $state([]);

    constructor(pubkey: string) {
        this.user = userFromPubkey(pubkey);
        if (this.user) {
            this.fetchUserFollows();
        }
    }

    async fetchUserFollows(): Promise<string[]> {
        if (!this.user) return [];

        const follows = await loadFollowPubkeys(this.user.pubkey);
        this.follows = follows;
        return follows;
    }
}

export function getCurrentUser(): CurrentUser | null {
    return currentUser;
}

export function setCurrentUser(pubkey: string | null): CurrentUser | null {
    currentUser = pubkey ? new CurrentUser(pubkey) : null;
    return currentUser;
}
