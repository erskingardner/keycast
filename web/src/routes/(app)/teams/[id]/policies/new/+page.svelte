<script lang="ts">
import { goto } from "$app/navigation";
import { page } from "$app/stores";
import PageSection from "$lib/components/PageSection.svelte";
import PermissionCard from "$lib/components/PermissionCard.svelte";
import PermissionForm from "$lib/components/PermissionForm.svelte";
import { getCurrentUser } from "$lib/current_user.svelte";
import { KeycastApi } from "$lib/keycast_api.svelte";
import ndk from "$lib/ndk.svelte";
import type {
    AllowedKindsConfig,
    ContentFilterConfig,
    Permission,
} from "$lib/types";
import { NDKNip07Signer } from "@nostr-dev-kit/ndk";
import { toast } from "svelte-hot-french-toast";

const { id } = $page.params;

const api = new KeycastApi();
const user = $derived(getCurrentUser()?.user);
let policyName: string = $state("");
let permissions: Permission[] = $state([]);

let identifier: string | null = $state(null);
let config: AllowedKindsConfig | ContentFilterConfig | null = $state(null);

async function createPolicy() {
    if (!user) {
        toast.error("You must be logged in to create a policy");
        return;
    }

    if (permissions.length === 0) {
        toast.error("You must add at least one permission");
        return;
    }

    if (!policyName) {
        toast.error("You must enter a policy name");
        return;
    }

    const request = {
        name: policyName,
        permissions,
    };

    const authEvent = await api.buildUnsignedAuthEvent(
        `/teams/${id}/policies`,
        "POST",
        user?.pubkey,
        JSON.stringify(request),
    );

    if (!ndk.signer) {
        ndk.signer = new NDKNip07Signer();
    }

    await authEvent?.sign();

    api.post(`/teams/${id}/policies`, request, {
        headers: {
            Authorization: `Nostr ${btoa(JSON.stringify(authEvent))}`,
        },
    })
        .then((policy) => {
            toast.success("Policy created successfully");
            goto(`/teams/${id}`);
        })
        .catch((error) => {
            toast.error("Failed to create policy");
            toast.error(`Failed to create policy: ${error.message}`);
        });
}

async function addPermission() {
    if (!identifier) {
        toast.error("Permission identifier required");
        return;
    }
    permissions.push({ identifier: identifier as string, config });
    identifier = null;
    config = null;
    toast.success("Permission added");
}
</script>

<h1 class="page-header flex flex-row gap-1 items-center">
    Add Policy
</h1>

<form class="flex flex-col gap-4">
    <div class="form-group">
        <label for="policyName">Policy name</label>
        <input type="text" id="policyName" bind:value={policyName} placeholder="My policy..." />
    </div>
    <PageSection title="Permissions">
        {#if permissions.length === 0}
            <p class="text-gray-400">No permissions added yet</p>
        {:else}
            <div class="card-grid">
                {#each permissions as permission}
                    <PermissionCard {permission} />
                {/each}
            </div>
        {/if}
    </PageSection>
    <PageSection title="Add a new permission">
        <PermissionForm bind:identifier bind:config />
        <button onclick={addPermission} class="button button-secondary self-start mt-6 mb-6">Add permission</button>
    </PageSection>
    
    <button onclick={createPolicy} class="button button-primary self-start">Save Policy</button>
</form>
