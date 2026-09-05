<script lang="ts">
import { goto } from "$app/navigation";
import { page } from "$app/stores";
import { getCurrentUser } from "$lib/current_user.svelte";
import { KeycastApi } from "$lib/keycast_api.svelte";
import type { User } from "$lib/types";
import { userFromPubkeyOrNpub } from "$lib/utils/nostr";
import { toast } from "svelte-hot-french-toast";

const { id } = $page.params;

const api = new KeycastApi();
const user = $derived(getCurrentUser()?.user);

let pubkeyOrNpub: string = $state("");
let role: "admin" | "member" = $state("member");
let errorMessage: string | null = $state(null);

async function addTeammate() {
    if (!user?.pubkey) return;
    if (!pubkeyOrNpub) {
        errorMessage = "You must provide a public key or npub.";
        return;
    }

    const teammate = userFromPubkeyOrNpub(pubkeyOrNpub);

    if (!teammate) {
        errorMessage = "Invalid public key or npub.";
        return;
    }

    api.buildAuthHeader(
        `/teams/${id}/users`,
        "POST",
        user.pubkey,
        JSON.stringify({
            user_public_key: teammate.pubkey,
            role,
        }),
    ).then((authHeader) => {
        api.post<User>(
            `/teams/${id}/users`,
            {
                user_public_key: teammate.pubkey,
                role,
            },
            {
                headers: { Authorization: authHeader },
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
    });
}
</script>

<h1 class="page-header">Add Teammate</h1>

<form onsubmit={(event) => { event.preventDefault(); addTeammate(); }}>
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
            <option value="member">Member</option>
            <option value="admin">Admin</option>
        </select>
    </div>
    <button type="submit" class="button button-primary">Add Teammate</button>
</form>
