<script lang="ts">
import { goto } from "$app/navigation";
import { page } from "$app/stores";
import { getCurrentUser } from "$lib/current_user.svelte";
import { KeycastApi } from "$lib/keycast_api.svelte";
import type { StoredKey } from "$lib/types";
import { toast } from "svelte-hot-french-toast";

const { id } = $page.params;

const api = new KeycastApi();
const user = $derived(getCurrentUser()?.user);

let keyName: string = $state("");
let secretKey: string = $state("");
let keyError: string | null = $state(null);

async function createKey() {
    if (!user?.pubkey) return;
    if (!secretKey) {
        keyError = "You must provide a private key.";
        return;
    }
    if (!keyName) {
        keyError = "You must provide a key name.";
        return;
    }

    const request = {
        name: keyName,
        secret_key: secretKey,
    };

    secretKey = "";
    try {
        const authHeader = await api.buildAuthHeader(`/teams/${id}/keys`, "POST", user.pubkey, JSON.stringify(request));
        await api.post<StoredKey>(`/teams/${id}/keys`, request, { headers: { Authorization: authHeader } });
        toast.success("Key created successfully");
        await goto(`/teams/${id}`);
    } catch (error) {
        keyError = error instanceof Error ? error.message : "Key import failed";
        toast.error("Failed to create key");
    } finally {
        request.secret_key = "";
        secretKey = "";
    }

}
</script>

<h1 class="page-header">Add Key</h1>

<form onsubmit={(event) => { event.preventDefault(); createKey(); }}>
    <div class="form-group">
        <label for="keyName">Key Name</label>
        <input type="text" bind:value={keyName} />
    </div>
    <div class="form-group">
        <label for="secretKey">Private key (nsec or hex)</label>
        <input type="password" autocomplete="off" spellcheck="false" placeholder="nsec1..." bind:value={secretKey} />
        <p class="text-sm text-gray-400">Browser import trusts this page with your private key. For stronger protection, import with the local Keycast CLI on your server.</p>
    </div>

    <button type="submit" class="button button-primary">Import Key</button>
</form>
