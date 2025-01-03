<script lang="ts">
import { goto } from "$app/navigation";
import { page } from "$app/stores";
import { getCurrentUser } from "$lib/current_user.svelte";
import { KeycastApi } from "$lib/keycast_api.svelte";
import ndk from "$lib/ndk.svelte";
import type { User } from "$lib/types";
import { userFromPubkeyOrNpub } from "$lib/utils/nostr";
import { type NDKEvent, NDKNip07Signer } from "@nostr-dev-kit/ndk";
import { toast } from "svelte-hot-french-toast";

const { id } = $page.params;

const api = new KeycastApi();
const user = $derived(getCurrentUser()?.user);

let unsignedAuthEvent: NDKEvent | null = $state(null);
let pubkeyOrNpub: string = $state("");
let role: "Admin" | "Member" = $state("Member");
let errorMessage: string | null = $state(null);

async function addTeammate() {
    if (!user?.pubkey) return;
    if (!pubkeyOrNpub) {
        errorMessage = "You must provide a public key or npub.";
        return;
    }

    const ndkUser = userFromPubkeyOrNpub(pubkeyOrNpub);

    if (!ndkUser) {
        errorMessage = "Invalid public key or npub.";
        return;
    }

    api.buildUnsignedAuthEvent(
        `/teams/${id}/users`,
        "POST",
        user.pubkey,
        JSON.stringify({
            user_public_key: ndkUser.pubkey,
            role,
        }),
    ).then(async (event) => {
        unsignedAuthEvent = event;
        if (unsignedAuthEvent) {
            if (!ndk.signer) {
                ndk.signer = new NDKNip07Signer();
            }
            await unsignedAuthEvent.sign();
            console.log(unsignedAuthEvent);
            const encodedAuthEvent = `Nostr ${btoa(JSON.stringify(unsignedAuthEvent))}`;
            api.post<User>(
                `/teams/${id}/users`,
                {
                    user_public_key: ndkUser.pubkey,
                    role,
                },
                {
                    headers: { Authorization: encodedAuthEvent },
                },
            )
                .then((_newUser) => {
                    toast.success("Teammate added successfully");
                    goto(`/teams/${id}`);
                })
                .catch((error) => {
                    toast.error("Failed to add teammate");
                    errorMessage = error.message;
                });
        }
    });
}
</script>

<h1 class="page-header">Add Teammate</h1>

<form onsubmit={() => addTeammate()}>
    <div class="form-group">
        <label for="pubkey">Public key or npub</label>
        <input type="text" bind:value={pubkeyOrNpub} placeholder="npub1..." />
        {#if errorMessage}
            <span class="input-error">{errorMessage}</span>
        {/if}
    </div>
    <div class="form-group">
        <label for="role">Role</label>
        <select bind:value={role}>
            <option value="Member">Member</option>
            <option value="Admin">Admin</option>
        </select>
    </div>
    <button type="submit" class="button button-primary">Add Teammate</button>
</form>
