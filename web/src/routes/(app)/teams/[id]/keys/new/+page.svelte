<script lang="ts">
import { goto } from "$app/navigation";
import { page } from "$app/stores";
import { getCurrentUser } from "$lib/current_user.svelte";
import { KeycastApi } from "$lib/keycast_api.svelte";
import ndk from "$lib/ndk.svelte";
import type { StoredKey } from "$lib/types";
import { type NDKEvent, NDKNip07Signer } from "@nostr-dev-kit/ndk";
import { toast } from "svelte-hot-french-toast";

const { id } = $page.params;

const api = new KeycastApi();
const user = $derived(getCurrentUser()?.user);

let unsignedAuthEvent: NDKEvent | null = $state(null);
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

    api.buildUnsignedAuthEvent(
        `/teams/${id}/keys`,
        "POST",
        user.pubkey,
        JSON.stringify({
            key_name: keyName,
            secret_key: secretKey,
        }),
    ).then(async (event) => {
        unsignedAuthEvent = event;
        if (unsignedAuthEvent) {
            if (!ndk.signer) {
                ndk.signer = new NDKNip07Signer();
            }
            await unsignedAuthEvent.sign();
            const encodedAuthEvent = `Nostr ${btoa(JSON.stringify(unsignedAuthEvent))}`;
            api.post<StoredKey>(
                `/teams/${id}/keys`,
                { name: keyName, secret_key: secretKey },
                {
                    headers: { Authorization: encodedAuthEvent },
                },
            )
                .then((newKey) => {
                    toast.success("Key created successfully");
                    goto(`/teams/${id}`);
                })
                .catch((error) => {
                    toast.error("Failed to create key");
                    keyError = error.message;
                });
        }
    });
}
</script>

<h1 class="page-header">Add Key</h1>

<form onsubmit={() => createKey()}>
    <div class="form-group">
  <label for="keyName">Key Name</label>
        <input type="text" bind:value={keyName} />
    </div>
    <div class="form-group">
        <label for="secretKey">Private key (nsec or hex)</label>
        <input type="password" placeholder="nsec1..." bind:value={secretKey} />
    </div>

    <button type="submit" class="button button-primary">Securely Add Key</button>
</form>
