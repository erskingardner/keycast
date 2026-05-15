import { browser } from "$app/environment";
import { goto } from "$app/navigation";
import { getCurrentUser, setCurrentUser } from "$lib/current_user.svelte";
import type NDK from "@nostr-dev-kit/ndk";
import { NDKNip07Signer, type NDKUser } from "@nostr-dev-kit/ndk";
import toast from "svelte-hot-french-toast";
import { fetchPubkeyAllowlist, isPubkeyAllowed } from "./allowlist";

async function isAllowedPubkey(pubkey: string) {
    const allowedPubkeys = await fetchPubkeyAllowlist();
    return isPubkeyAllowed(pubkey, allowedPubkeys);
}

export async function signin(ndk: NDK): Promise<NDKUser | null> {
    const signedInUser = await userFromNip07(ndk);

    if (signedInUser) {
        let allowed = false;
        try {
            allowed = await isAllowedPubkey(signedInUser.pubkey);
        } catch (error) {
            toast.error("Unable to verify pubkey authorization");
            console.error(error);
            return null;
        }

        if (!allowed) {
            toast.error("Your pubkey is not authorized");
            return null;
        }
        signedInUser.ndk = ndk;
        ndk.activeUser = signedInUser;
        const alreadySignedIn = !!getCurrentUser();
        setCurrentUser(signedInUser.pubkey);
        document.cookie = `keycastUserPubkey=${signedInUser.pubkey}; max-age=1209600; SameSite=Lax; Secure; path=/`;
        if (!alreadySignedIn) {
            toast.success("Signed in successfully");
        }
        goto("/teams");
    }
    return signedInUser;
}

/**
 * Retrieves a user object using the raw NIP-07 browser extension API.
 * @async
 * @param ndk - An instance of the NDK class.
 * @returns A Promise that resolves to an NDKUser object if a NIP-07 extension is available, or null otherwise.
 */
async function userFromNip07(ndk: NDK): Promise<NDKUser | null> {
    if (!browser || !window.nostr) {
        toast.error("Install or enable a NIP-07 browser extension to sign in");
        return null;
    }

    try {
        const pubkey = await window.nostr.getPublicKey();
        if (!pubkey) {
            toast.error("The NIP-07 extension did not return a pubkey");
            return null;
        }

        ndk.signer = new NDKNip07Signer();
        return ndk.getUser({ pubkey });
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        toast.error(message);
        console.error(error);
        return null;
    }
}

/**
 * Signs the user out.
 */
export function signout(ndk: NDK) {
    setCurrentUser(null);
    ndk.activeUser = undefined;
    document.cookie = "keycastUserPubkey=";
    toast.success("Signed out");
    goto("/");
}
