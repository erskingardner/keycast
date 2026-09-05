<script lang="ts">
import { goto } from "$app/navigation";
import { page } from "$app/stores";
import Copy from "$lib/components/Copy.svelte";
import GrantCard from "$lib/components/GrantCard.svelte";
import Loader from "$lib/components/Loader.svelte";
import PageSection from "$lib/components/PageSection.svelte";
import { getCurrentUser } from "$lib/current_user.svelte";
import { KeycastApi } from "$lib/keycast_api.svelte";
import type { Grant, InvitationCreationResponse, KeyWithRelations, StoredKey, Team } from "$lib/types";
import { dateFromUnixSeconds, formattedDate } from "$lib/utils/dates";
import { CaretRight } from "phosphor-svelte";
import { toast } from "svelte-hot-french-toast";

const id = $page.params.id ?? "";
const pubkey = $page.params.pubkey ?? "";
const api = new KeycastApi();
const user = $derived(getCurrentUser()?.user);
let isLoading = $state(true);
let team: Team | null = $state(null);
let key: StoredKey | null = $state(null);
let grants: Grant[] = $state([]);
let invitationUri: string | null = $state(null);
let loadError: string | null = $state(null);

$effect(() => {
    if (!user?.pubkey || !isLoading) return;
    const endpoint = `/teams/${id}/keys/${pubkey}`;
    api.buildAuthHeader(endpoint, "GET", user.pubkey)
        .then((authorization) => api.get<KeyWithRelations>(endpoint, { headers: { Authorization: authorization } }))
        .then((response) => {
            key = response.stored_key;
            team = response.team;
            grants = response.grants;
        })
        .catch((error) => { loadError = error instanceof Error ? error.message : String(error); })
        .finally(() => { isLoading = false; });
});

async function removeKey() {
    if (!user?.pubkey || !confirm("Remove this key and revoke every grant and session attached to it?")) return;
    const endpoint = `/teams/${id}/keys/${pubkey}`;
    const authorization = await api.buildAuthHeader(endpoint, "DELETE", user.pubkey);
    await api.delete(endpoint, { headers: { Authorization: authorization } });
    toast.success("Key removed");
    await goto(`/teams/${id}`);
}

async function revokeGrant(grant: Grant) {
    if (!user?.pubkey || !confirm("Revoke this grant and all of its active sessions?")) return;
    const endpoint = `/teams/${id}/keys/${pubkey}/grants/${grant.id}`;
    const authorization = await api.buildAuthHeader(endpoint, "DELETE", user.pubkey);
    await api.delete(endpoint, { headers: { Authorization: authorization } });
    grants = grants.map((item) => item.id === grant.id ? { ...item, revoked_at: Math.floor(Date.now() / 1000), active_sessions: 0, claimable_invitations: 0 } : item);
    toast.success("Grant revoked");
}

async function createInvitation(grant: Grant) {
    if (!user?.pubkey) return;
    const endpoint = `/teams/${id}/grants/${grant.id}/invitations`;
    const request = { expires_at: Math.floor(Date.now() / 1000) + 24 * 3600 };
    const body = JSON.stringify(request);
    const authorization = await api.buildAuthHeader(endpoint, "POST", user.pubkey, body);
    const response = await api.post<InvitationCreationResponse>(endpoint, request, { headers: { Authorization: authorization } });
    invitationUri = response.bunker_uri;
    grants = grants.map((item) => item.id === grant.id ? { ...item, claimable_invitations: item.claimable_invitations + 1 } : item);
    toast.success("One-time invitation created");
}
</script>

{#if isLoading}
    <Loader extraClasses="items-center justify-center mt-40" />
{:else if loadError}
    <p class="input-error">{loadError}</p>
{:else if team && key}
    <h1 class="page-header flex flex-row gap-1 items-center">
        <a href={`/teams/${id}`} class="bordered">{team.name}</a>
        <CaretRight size="20" class="text-gray-500" /> {key.name}
    </h1>

    <div class="card mb-6">
        <span class="font-mono text-xs break-all flex items-center gap-2">{key.public_key}<Copy value={key.public_key} /></span>
        <span class="text-xs text-gray-400">Added {formattedDate(dateFromUnixSeconds(key.created_at))}</span>
    </div>

    {#if invitationUri}
        <div class="card mb-6 border border-amber-500/60">
            <h2 class="font-semibold text-amber-300">Copy this invitation now</h2>
            <p class="text-sm text-gray-300">Its secret is not stored and cannot be shown again.</p>
            <div class="font-mono text-xs break-all flex items-center gap-2 bg-gray-950 p-3 rounded">
                <span class="grow">{invitationUri}</span><Copy value={invitationUri} />
            </div>
            <button type="button" class="button button-secondary self-start" onclick={() => invitationUri = null}>Dismiss</button>
        </div>
    {/if}

    <PageSection title="Remote-signing grants">
        <div class="flex flex-col gap-4 items-start">
            {#if grants.length === 0}
                <p class="text-gray-500">No grants found</p>
            {:else}
                <div class="card-grid w-full">
                    {#each grants as grant}
                        <GrantCard {grant} onRevoke={revokeGrant} onCreateInvitation={createInvitation} />
                    {/each}
                </div>
            {/if}
            <a href={`/teams/${id}/keys/${pubkey}/grants/new`} class="button button-primary">Add grant</a>
        </div>
    </PageSection>

    <PageSection title="Danger Zone">
        <button onclick={removeKey} class="button button-danger">Remove key from team</button>
    </PageSection>
{/if}
