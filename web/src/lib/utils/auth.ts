import { goto } from "$app/navigation";
import { getCurrentUser, setCurrentUser } from "$lib/current_user.svelte";
import {
    clearSignerSession,
    getAmberPubkey,
    getExtensionPubkey,
    getRemoteSignerPubkey,
    hasNip07Extension,
    userFromPubkey,
    type NostrUser,
} from "$lib/nostr";
import toast from "svelte-hot-french-toast";
import { checkPubkeyAllowed } from "./allowlist";

export type SigninMethod = "extension" | "amber" | "remote";

async function isAllowedPubkey(pubkey: string) {
    return checkPubkeyAllowed(pubkey);
}

export async function signin(
    method: SigninMethod = "extension",
): Promise<NostrUser | null> {
    const signedInUser = await userFromSigner(method);

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
 * Retrieves a user object using the selected external signer.
 * @async
 * @returns A Promise that resolves to a Nostr user if the signer returns a valid pubkey, or null otherwise.
 */
async function userFromSigner(method: SigninMethod): Promise<NostrUser | null> {
    if (method === "extension" && !hasNip07Extension()) {
        toast.error("Install or enable a NIP-07 browser extension to sign in");
        return null;
    }

    try {
        const user = userFromPubkey(await getPubkeyForMethod(method));
        if (!user) {
            toast.error("The signer did not return a valid pubkey");
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

async function getPubkeyForMethod(method: SigninMethod): Promise<string> {
    if (method === "extension") {
        return getExtensionPubkey();
    }

    if (method === "amber") {
        return getAmberPubkey();
    }

    const bunkerUri = window.prompt("Paste your bunker:// remote signer URI");
    if (!bunkerUri) {
        throw new Error("Remote signer URI was not provided");
    }

    return getRemoteSignerPubkey(bunkerUri.trim());
}

/**
 * Signs the user out.
 */
export function signout() {
    setCurrentUser(null);
    clearSignerSession();
    document.cookie = "keycastUserPubkey=";
    toast.success("Signed out");
    goto("/");
}
