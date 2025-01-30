<script lang="ts">
import { goto } from "$app/navigation";
import { page } from "$app/stores";
import PageSection from "$lib/components/PageSection.svelte";
import { getCurrentUser } from "$lib/current_user.svelte";
import { KeycastApi } from "$lib/keycast_api.svelte";
import ndk from "$lib/ndk.svelte";
import type {
    PolicyWithPermissions,
    StoredKey,
    Team,
    TeamWithRelations,
} from "$lib/types";
import { readablePermissionConfig } from "$lib/utils/permissions";
import { toTitleCase } from "$lib/utils/strings";
import { type NDKEvent, NDKNip07Signer } from "@nostr-dev-kit/ndk";
import { CaretRight, X } from "phosphor-svelte";
import { toast } from "svelte-hot-french-toast";

const { id, pubkey } = $page.params;

const api = new KeycastApi();
const user = $derived(getCurrentUser()?.user);
let isLoading = $state(true);
let unsignedAuthEvent: NDKEvent | null = $state(null);
let encodedAuthEvent: string | null = $state(null);

let maxUses: number | null = $state(0);
let expiresAt: Date | null = $state(null);
let relaysString: string = $state(
    "wss://relay.nsecbunker.com, wss://relay.nsec.app",
);

let relays: string[] = $derived(
    relaysString.split(",").map((relay) => relay.trim()),
);

let teamWithRelations: TeamWithRelations | null = $state(null);
let team: Team | null = $state(null);
let policies: PolicyWithPermissions[] | null = $state(null);
let key: StoredKey | null | undefined = $state(null);
let selectedPolicyId: number | null = $state(null);

let readyToSubmit = $derived(
    maxUses !== null && relaysString && selectedPolicyId,
);

$effect(() => {
    if (user?.pubkey && !unsignedAuthEvent) {
        api.buildUnsignedAuthEvent(`/teams/${id}`, "GET", user.pubkey).then(
            async (event) => {
                unsignedAuthEvent = event;
                if (unsignedAuthEvent) {
                    if (!ndk.signer) {
                        ndk.signer = new NDKNip07Signer();
                    }
                    await unsignedAuthEvent.sign();
                    encodedAuthEvent = `Nostr ${btoa(JSON.stringify(unsignedAuthEvent))}`;
                    api.get(`/teams/${id}`, {
                        headers: { Authorization: encodedAuthEvent },
                    })
                        .then((teamResponse) => {
                            teamWithRelations =
                                teamResponse as TeamWithRelations;
                            team = teamWithRelations.team;
                            key = teamWithRelations.stored_keys.find(
                                (key) => key.public_key === pubkey,
                            );
                            policies = teamWithRelations.policies;
                        })
                        .finally(() => {
                            isLoading = false;
                        });
                }
            },
        );
    }
});

async function createAuthorization() {
    if (!user?.pubkey) {
        toast.error("You must be logged in to create an authorization");
        return;
    }

    if (!selectedPolicyId) {
        toast.error("You must select a policy");
        return;
    }

    if (!relaysString) {
        toast.error("You must enter at least one relay");
        return;
    }

    const request = {
        max_uses: maxUses === 0 ? null : maxUses,
        expires_at: expiresAt
            ? Math.floor(new Date(expiresAt).getTime() / 1000)
            : null,
        relays: relays,
        policy_id: selectedPolicyId,
    };

    const authEvent = await api.buildUnsignedAuthEvent(
        `/teams/${id}/keys/${pubkey}/authorizations`,
        "POST",
        user?.pubkey,
        JSON.stringify(request),
    );

    if (!ndk.signer) {
        ndk.signer = new NDKNip07Signer();
    }

    await authEvent?.sign();

    api.post(`/teams/${id}/keys/${pubkey}/authorizations`, request, {
        headers: {
            Authorization: `Nostr ${btoa(JSON.stringify(authEvent))}`,
        },
    })
        .then((_authorization) => {
            toast.success("Authorization created successfully");
            goto(`/teams/${id}/keys/${pubkey}`);
        })
        .catch((error) => {
            toast.error("Failed to create authorization");
            toast.error(`Failed to create authorization: ${error.message}`);
        });
}
</script>

<h1 class="page-header flex flex-row gap-1 items-center">
    <a href={`/teams/${id}`} class="bordered">{team?.name}</a>
    <CaretRight size="20" class="text-gray-500" />
    <a href={`/teams/${id}/keys/${pubkey}`} class="bordered">{key?.name}</a>
    <CaretRight size="20" class="text-gray-500" />
    Add Authorization
</h1>

<PageSection title="Authorization">
    <form onsubmit={() => createAuthorization()}>
        <div class="form-group">
            <label for="maxUses">Maximum uses (Zero for unlimited)</label>
            <input type="number" bind:value={maxUses} />
        </div>

        <div class="form-group">
            <label for="relays">Relays (Comma separated)</label>
            <input type="text" bind:value={relaysString} />
        </div>

        <div class="form-group">
            <label for="expiresAt">Expiration date (Leave blank for no expiration)</label>
            <div class="flex flex-row gap-2 items-center">
                <input type="datetime-local" bind:value={expiresAt} />
                {#if expiresAt}
                    <button type="button" class="clear-button" onclick={() => expiresAt = null}>
                        <X weight="light" size={16} />
                    </button>
                {/if}
            </div>
        </div>
    </form>
</PageSection>

    <PageSection title="Policies">
        <div class="flex flex-col gap-4">
            {#if !policies || policies.length === 0}
                <p class="text-gray-500">No policies found</p>
            {:else}
                <div class="card-grid">
                    {#each policies as policy}
                        <!-- svelte-ignore a11y_click_events_have_key_events -->
                        <div
                            class="card hover-card {selectedPolicyId === policy.policy.id ? '!ring-2 !ring-indigo-500' : ''}"
                            onclick={() => selectedPolicyId = policy.policy.id}
                            role="button"
                            tabindex="0"
                        >
                            <h3 class="text-lg font-semibold">{policy.policy.name}</h3>
                            <ul class="">
                                {#each policy.permissions as permission}
                                    <li class="text-sm text-gray-300">{toTitleCase(permission.identifier)}
                                        <ul class="list-disc list-inside ml-2">
                                            {#each readablePermissionConfig(permission) as config}
                                                <li class="text-xs text-gray-400">{config}</li>
                                            {/each}
                                        </ul>
                                    </li>
                                {/each}
                            </ul>
                        </div>
                    {/each}
                </div>
            {/if}
            <a href={`/teams/${id}/policies/new`} class="button self-start button-primary !my-0">Add Policy</a>
        </div>
    </PageSection>

    <button onclick={createAuthorization} class="button button-primary" disabled={!readyToSubmit}>Add Authorization</button>
