import { goto } from "$app/navigation";
import { getCurrentUser, setCurrentUser } from "$lib/current_user.svelte";
import {
    getExtensionPubkey,
    hasNip07Extension,
    userFromPubkey,
    type NostrUser,
} from "$lib/nostr";
import toast from "svelte-hot-french-toast";
import { checkPubkeyAllowed } from "./allowlist";

async function isAllowedPubkey(pubkey: string) {
    return checkPubkeyAllowed(pubkey);
}

export async function signin(): Promise<NostrUser | null> {
    const signedInUser = await userFromNip07();

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
 * @returns A Promise that resolves to a Nostr user if a NIP-07 extension is available, or null otherwise.
 */
async function userFromNip07(): Promise<NostrUser | null> {
    if (!hasNip07Extension()) {
        toast.error("Install or enable a NIP-07 browser extension to sign in");
        return null;
    }

    try {
        const user = userFromPubkey(await getExtensionPubkey());
        if (!user) {
            toast.error("The NIP-07 extension did not return a valid pubkey");
            return null;
        }

        return user;
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
export function signout() {
    setCurrentUser(null);
    document.cookie = "keycastUserPubkey=";
    toast.success("Signed out");
    goto("/");
}
