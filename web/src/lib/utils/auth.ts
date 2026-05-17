import { goto } from "$app/navigation";
import { getCurrentUser, setCurrentUser } from "$lib/current_user.svelte";
import {
    clearActiveSigner,
    connectNostrConnectBunker,
    getAmberUser,
    getExtensionUser,
    isAmberSigninSupported,
    type NostrUser,
} from "$lib/nostr";
import toast from "svelte-hot-french-toast";
import { checkPubkeyAllowed } from "./allowlist";

export type SigninMethod = "extension" | "nip46-bunker" | "amber";
export type SigninOptions = {
    bunkerUri?: string;
};

async function isAllowedPubkey(pubkey: string) {
    return checkPubkeyAllowed(pubkey);
}

export async function signin(
    method: SigninMethod = "extension",
    options: SigninOptions = {},
): Promise<NostrUser | null> {
    const signedInUser = await userFromSigninMethod(method, options);
    return completeSignin(signedInUser);
}

export async function completeSignin(
    signedInUser: NostrUser | null,
): Promise<NostrUser | null> {
    if (signedInUser) {
        let allowed = false;
        try {
            allowed = await isAllowedPubkey(signedInUser.pubkey);
        } catch (error) {
            clearActiveSigner();
            toast.error("Unable to verify pubkey authorization");
            console.error(error);
            return null;
        }

        if (!allowed) {
            clearActiveSigner();
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

async function userFromSigninMethod(
    method: SigninMethod,
    options: SigninOptions,
): Promise<NostrUser | null> {
    try {
        switch (method) {
            case "extension":
                return getExtensionUser();
            case "nip46-bunker":
                if (!options.bunkerUri) {
                    toast.error("Paste a bunker:// remote signer connection string");
                    return null;
                }
                return connectNostrConnectBunker(options.bunkerUri);
            case "amber":
                if (!isAmberSigninSupported()) {
                    toast.error("Amber sign-in is only available on supported Android browsers");
                    return null;
                }
                return getAmberUser();
            default:
                method satisfies never;
                return null;
        }
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
    clearActiveSigner();
    setCurrentUser(null);
    document.cookie = "keycastUserPubkey=; max-age=0; SameSite=Lax; Secure; path=/";
    toast.success("Signed out");
    goto("/");
}
