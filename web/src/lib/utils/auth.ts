import { browser } from "$app/environment";
import { goto } from "$app/navigation";
import { getCurrentUser, setCurrentUser } from "$lib/current_user.svelte";
import type NDK from "@nostr-dev-kit/ndk";
import {
    NDKNip07Signer,
    NDKNip46Signer,
    NDKPrivateKeySigner,
    type NDKUser,
} from "@nostr-dev-kit/ndk";
import toast from "svelte-hot-french-toast";

export enum SigninMethod {
    Nip07 = "nip07",
    NostrLogin = "nostr-login",
    // Nip46 = "nip46",
    // PK = "pk",
}

function isAllowedPubkey(pubkey: string) {
    return JSON.stringify(import.meta.env.VITE_ALLOWED_PUBKEYS).includes(
        pubkey,
    );
}

/**
 * Attempt to signin with the same method that was previously used, or default to NIP-07 extension
 */
export async function signin(
    ndk: NDK,
    bunkerNDK?: NDK,
    method?: SigninMethod,
    token?: string,
    user?: NDKUser,
): Promise<NDKUser | null> {
    // We only handle NIP-07 or nostr login for now
    let signedInUser: NDKUser | null = user || null;
    if (method === SigninMethod.Nip07) {
        signedInUser = await userFromNip07(ndk);
    }
    if (signedInUser) {
        if (!isAllowedPubkey(signedInUser.pubkey)) {
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

// /**
//  * Retrieves a user object using a private key stored in local storage
//  * Don't use this.
//  * @async
//  * @param ndk - An instance of the NDK class.
//  * @returns A Promise that resolves to an NDKUser object if the private key is found, or null otherwise.
//  */
// async function userFromPrivateKey(ndk: NDK): Promise<NDKUser | null> {
//     const privateKey = localStorage.getItem("listrNostrPrivateKey");
//     if (!privateKey) return null;

//     const signer = new NDKPrivateKeySigner(privateKey);
//     ndk.signer = signer;

//     const user = await signer.user();
//     localStorage.setItem("nostrSigninMethod", SigninMethod.PK);
//     return user;
// }

/**
 * Retrieves a user object using the NDKNip07Signer.
 * @async
 * @param ndk - An instance of the NDK class.
 * @returns A Promise that resolves to an NDKUser object if the NDKNip07Signer is available, or null otherwise.
 */
async function userFromNip07(ndk: NDK): Promise<NDKUser | null> {
    let user: NDKUser | null = null;
    if (browser && window.nostr) {
        try {
            ndk.signer = new NDKNip07Signer();
            user = await ndk.signer.user();
        } catch (error) {
            toast.error(error as string);
            console.error(error);
        }
    }
    return user;
}

// /**
//  * Retrieves a user object using the NDKNip46Signer.
//  * @async
//  * @param ndk - An instance of the NDK class.
//  * @param bunkerNdk - An instance of the NDK class for the Bunker server.
//  * @param token - An optional token for creating a new nsecBunker connection.
//  * @returns A Promise that resolves to an NDKUser object if the NDKNip46Signer is available, or null otherwise.
//  */
// async function userFromNip46(ndk: NDK, bunkerNdk: NDK, token?: string): Promise<NDKUser | null> {
//     let localSigner: NDKPrivateKeySigner;
//     let user: NDKUser | null = null;
//     if (browser) {
//         const storedKey = localStorage.getItem("listrNip46LocalSignerPK");
//         const targetNpub = localStorage.getItem("listrNip46TargetNpub");
//         // If we have a local PK and a target npub, try and sign in.
//         if (storedKey && targetNpub) {
//             console.log("stored key and target npub");
//             localSigner = new NDKPrivateKeySigner(storedKey);
//             const targetUser = ndk.getUser({ npub: targetNpub });
//             const remoteSigner = new NDKNip46Signer(bunkerNdk, targetUser.pubkey, localSigner);
//             ndk.signer = remoteSigner;
//             await remoteSigner.blockUntilReady();
//             user = await remoteSigner.user();
//         }
//         // If we're missing one of the above but we have a token, try and create a new nsecBunker connection
//         else if (token) {
//             const localSigner = NDKPrivateKeySigner.generate();
//             localStorage.setItem("listrNip46LocalSignerPK", localSigner.privateKey as string);
//             const remoteSigner = new NDKNip46Signer(bunkerNdk, token, localSigner);
//             ndk.signer = remoteSigner;
//             try {
//                 await remoteSigner.blockUntilReady();
//                 user = await remoteSigner.user();
//                 localStorage.setItem("listrNip46TargetNpub", user.npub);
//                 // eslint-disable-next-line @typescript-eslint/no-explicit-any
//             } catch (error) {
//                 toast.error(error as string);
//                 console.error(error);
//             }
//         }
//         localStorage.setItem("nostrSigninMethod", SigninMethod.Nip46);
//     }
//     return user;
// }

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
