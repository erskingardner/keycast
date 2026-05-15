<script lang="ts">
import { goto } from "$app/navigation";
import { page } from "$app/stores";
import AuthorizationCard from "$lib/components/AuthorizationCard.svelte";
import Avatar from "$lib/components/Avatar.svelte";
import Copy from "$lib/components/Copy.svelte";
import Loader from "$lib/components/Loader.svelte";
import Name from "$lib/components/Name.svelte";
import PageSection from "$lib/components/PageSection.svelte";
import { getCurrentUser } from "$lib/current_user.svelte";
import { KeycastApi } from "$lib/keycast_api.svelte";
import {
    loadProfile,
    userFromPubkey,
    type NostrProfile,
} from "$lib/nostr";
import type {
    AuthorizationWithRelations,
    KeyWithRelations,
    StoredKey,
    Team,
} from "$lib/types";
import { formattedDate } from "$lib/utils/dates";
import { safeRemoteImageUrl } from "$lib/utils/image_url";
import { CaretRight } from "phosphor-svelte";
import { toast } from "svelte-hot-french-toast";

const id = $page.params.id ?? "";
const pubkey = $page.params.pubkey ?? "";

const api = new KeycastApi();
const user = $derived(getCurrentUser()?.user);
let isLoading = $state(true);
let keyAuthHeader: string | null = $state(null);
let team: Team | null = $state(null);
let key: StoredKey | null = $state(null);
let authorizations: AuthorizationWithRelations[] = $state([]);
let keyUser = $derived(userFromPubkey(pubkey));
let keyUserProfile = $state<NostrProfile | null>(null);
let keyUserBannerUrl = $derived(safeRemoteImageUrl(keyUserProfile?.banner));

$effect(() => {
    if (user?.pubkey && !keyAuthHeader) {
        api.buildAuthHeader(
            `/teams/${id}/keys/${pubkey}`,
            "GET",
            user.pubkey,
        )
            .then((authHeader) => {
                keyAuthHeader = authHeader;
                return api.get(`/teams/${id}/keys/${pubkey}`, {
                    headers: { Authorization: authHeader },
                });
            })
            .then((teamKeyResponse) => {
                key = (teamKeyResponse as KeyWithRelations).stored_key;
                team = (teamKeyResponse as KeyWithRelations).team;
                authorizations = (teamKeyResponse as KeyWithRelations)
                    .authorizations;
            })
            .finally(() => {
                isLoading = false;
            });
    }

    if (key && !keyUserProfile) {
        loadProfile(pubkey).then((profile) => {
            keyUserProfile = profile;
        });
    }
});

async function removeKey() {
    if (!user?.pubkey) return;
    if (
        !confirm(
            "Are you sure you want to remove this key from the team?\n\nThis will remove all authorizations associated with this key.",
        )
    )
        return;

    const authHeader = await api.buildAuthHeader(
        `/teams/${id}/keys/${pubkey}`,
        "DELETE",
        user?.pubkey,
    );

    api.delete(`/teams/${id}/keys/${pubkey}`, {
        headers: {
            Authorization: authHeader,
        },
    })
        .then(() => {
            toast.success("Key removed successfully");
            goto(`/teams/${id}`);
        })
        .catch((error) => {
            toast.error("Failed to remove key");
        });
}
</script>

{#if isLoading}
    <Loader extraClasses="items-center justify-center mt-40" />
{:else if team &&key}
    <h1 class="page-header flex flex-row gap-1 items-center">
        <a href={`/teams/${id}`} class="bordered">{team.name}</a>
        <CaretRight size="20" class="text-gray-500" />
        {key.name}
    </h1>
    <div
        class="relative"
    >
        <div class="absolute inset-0 bg-cover bg-center bg-gray-800 overflow-hidden rounded-lg">
            {#if keyUserBannerUrl}
                <img src={keyUserBannerUrl} alt="Banner" referrerpolicy="no-referrer" class="opacity-20 w-full h-full object-cover object-center rounded-lg" />
            {:else}
                <div class="w-full h-full bg-gray-800"></div>
            {/if}
        </div>
        <div class="relative p-6 flex items-center gap-4">
            <Avatar {pubkey} userProfile={keyUserProfile} extraClasses="w-24 h-24" />
            <div class="flex flex-col gap-1 truncate">
                <span class="font-semibold text-lg">
                    <Name {pubkey} userProfile={keyUserProfile} />
                </span>
                <span class="text-xs font-mono text-gray-300 flex flex-row gap-2 items-center justify-between truncate">
                    <span class="truncate">{keyUser?.npub}</span>
                    <Copy value={keyUser?.npub || ""} size="18" />
                </span>
                <span class="text-xs font-mono text-gray-300 flex flex-row gap-2 items-center justify-between truncate">
                    <span class="truncate">{keyUser?.pubkey}</span>
                    <Copy value={keyUser?.pubkey || ""} size="18" />
                </span>
                <span class="text-xs font-mono text-gray-400 mt-2">
                    Added: {formattedDate(new Date(key.created_at))}
                </span>
            </div>
        </div>
    </div>


    <PageSection title="Key Authorizations">
        <div class="flex flex-col gap-4 items-start">
            {#if authorizations.length === 0}
                <p class="text-gray-500">No authorizations found</p>
            {:else}
                <div class="card-grid">
                    {#each authorizations as authorization}
                        <AuthorizationCard {authorization} />
                    {/each}
                </div>
            {/if}
            <a href={`/teams/${id}/keys/${pubkey}/authorizations/new`} class="button button-primary">Add Authorization</a>
        </div>
    </PageSection>

    <PageSection title="Danger Zone">
        <button onclick={removeKey} class="button button-danger">Remove key from team</button>
    </PageSection>
{/if}
