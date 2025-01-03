import ndk from "$lib/ndk.svelte";
import type { NDKRelay, NDKUser } from "@nostr-dev-kit/ndk";

let currentUser: CurrentUser | null = $state(null);

class CurrentUser {
    /** The NDKUser instance representing the current logged in user */
    user: NDKUser | null = $state(null);

    /** Array of pubkeys that the current user follows */
    follows: string[] = $state([]);

    constructor(pubkey: string) {
        this.user = ndk.getUser({ pubkey });
        if (this.user) {
            this.fetchUserFollows();
        }
    }

    async fetchUserFollows(): Promise<string[]> {
        if (this.user) {
            const followsSet = await this.user.follows();
            const follows = Array.from(followsSet).map((user) => user.pubkey);
            this.follows = follows;
            return follows;
        }
        return Promise.resolve([]);
    }

    async follow(user: NDKUser): Promise<boolean> {
        if (!this.user) return false;
        const result = await this.user.follow(user);
        if (result) {
            this.follows = [...this.follows, user.pubkey];
        }
        return result;
    }

    async unfollow(user: NDKUser): Promise<boolean | Set<NDKRelay>> {
        if (!this.user) return false;
        const result = await this.user.unfollow(user);
        if (result) {
            this.follows = this.follows.filter(
                (pubkey) => pubkey !== user.pubkey,
            );
        }
        return result;
    }
}

export function getCurrentUser(): CurrentUser | null {
    return currentUser;
}

export function setCurrentUser(npub: string | null): CurrentUser | null {
    currentUser = npub ? new CurrentUser(npub) : null;
    return currentUser;
}
