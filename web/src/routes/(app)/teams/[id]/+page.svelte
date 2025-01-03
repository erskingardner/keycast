<script lang="ts">
import { goto } from "$app/navigation";
import { page } from "$app/stores";
import AdminPill from "$lib/components/AdminPill.svelte";
import Avatar from "$lib/components/Avatar.svelte";
import Loader from "$lib/components/Loader.svelte";
import Name from "$lib/components/Name.svelte";
import PageSection from "$lib/components/PageSection.svelte";
import PolicyCard from "$lib/components/PolicyCard.svelte";
import { getCurrentUser } from "$lib/current_user.svelte";
import { KeycastApi } from "$lib/keycast_api.svelte";
import ndk from "$lib/ndk.svelte";
import type {
    PolicyWithPermissions,
    StoredKey,
    TeamWithRelations,
    User,
} from "$lib/types";
import { truncatedNpubForPubkey } from "$lib/utils/nostr";
import { type NDKEvent, NDKNip07Signer } from "@nostr-dev-kit/ndk";
import { DotsThreeVertical } from "phosphor-svelte";
import { toast } from "svelte-hot-french-toast";

const { id } = $page.params;

const api = new KeycastApi();
const user = $derived(getCurrentUser()?.user);
let isLoading = $state(true);
let unsignedAuthEvent: NDKEvent | null = $state(null);
let encodedAuthEvent: string | null = $state(null);
let team: TeamWithRelations | null = $state(null);
let users: User[] = $state([]);
let storedKeys: StoredKey[] = $state([]);
let policies: PolicyWithPermissions[] = $state([]);

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
                            team = teamResponse as TeamWithRelations;
                            users = team.team_users;
                            storedKeys = team.stored_keys;
                            policies = team.policies;
                        })
                        .finally(() => {
                            isLoading = false;
                        });
                }
            },
        );
    }
});

async function deleteTeam() {
    if (!user?.pubkey) return;
    if (
        confirm(
            "Are you sure you want to delete this team? This action is irreversible.",
        )
    ) {
        const authEvent = await api.buildUnsignedAuthEvent(
            `/teams/${id}`,
            "DELETE",
            user?.pubkey,
        );
        if (!ndk.signer) {
            ndk.signer = new NDKNip07Signer();
        }
        await authEvent?.sign();

        api.delete(`/teams/${id}`, {
            headers: {
                Authorization: `Nostr ${btoa(JSON.stringify(authEvent))}`,
            },
        }).then(() => {
            toast.success("Team deleted successfully");
            goto("/teams");
        });
    }
}

async function showUserMenu(user: User) {
    const menu = document.getElementById(`user-menu-${user.user_public_key}`);
    if (menu) {
        menu.classList.toggle("hidden");
    }
}

async function removeUser(userToRemove: User) {
    if (!user?.pubkey) return;
    if (!confirm("Are you sure you want to remove this user?")) return;

    const authEvent = await api.buildUnsignedAuthEvent(
        `/teams/${id}/users/${userToRemove.user_public_key}`,
        "DELETE",
        user?.pubkey,
    );
    if (!ndk.signer) {
        ndk.signer = new NDKNip07Signer();
    }
    await authEvent?.sign();

    api.delete(`/teams/${id}/users/${userToRemove.user_public_key}`, {
        headers: {
            Authorization: `Nostr ${btoa(JSON.stringify(authEvent))}`,
        },
    })
        .then(() => {
            toast.success("User removed successfully");
            users = users.filter(
                (user) => user.user_public_key !== userToRemove.user_public_key,
            );
        })
        .catch((error) => {
            toast.error("Failed to remove user");
        });
}
</script>

{#if isLoading}
    <Loader extraClasses="items-center justify-center mt-40" />
{:else if team}
    <h1 class="page-header">{team?.team.name}</h1>

    <PageSection title="Members">
        <div class="card-grid mb-4">
            {#each users as user}
                <div class="card flex !flex-row gap-4 relative">
                    <Avatar user={ndk.getUser({ pubkey: user.user_public_key })} extraClasses="w-12 h-12" />
                    <div class="flex flex-col gap-1">
                        <span class="font-semibold">
                            <Name user={ndk.getUser({ pubkey: user.user_public_key })} />
                        </span>
                        <span class="font-mono text-xs text-gray-500">
                            {truncatedNpubForPubkey(user.user_public_key)}&hellip;
                        </span>
                    </div>
                    <AdminPill {user} />
                    <button onclick={() => showUserMenu(user)} class="absolute top-1.5 right-1"><DotsThreeVertical size={20} weight="bold" class="text-gray-500 hover:text-gray-200" /></button>
                    <div id={`user-menu-${user.user_public_key}`} class="hidden absolute top-8 right-1 bg-gray-700 ring-1 ring-gray-600 shadow-lg rounded-md p-2 text-sm">
                        <button onclick={() => removeUser(user)} class="text-gray-200 hover:text-white">Remove User</button>
                    </div>
                </div>
            {/each}
        </div>
        <a href={`/teams/${id}/users/new`} class="button button-primary">Add Member</a>
    </PageSection>


    <PageSection title="Keys">
        <div class="flex flex-col gap-4 items-start">
            {#if storedKeys.length === 0}
                <p class="text-gray-500">No keys found</p>
            {:else}
                <div class="card-grid">
                    {#each storedKeys as key}
                        <a href={`/teams/${id}/keys/${key.public_key}`} class="card hover-card flex !flex-row gap-4 ">
                            <Avatar user={ndk.getUser({ pubkey: key.public_key })} extraClasses="w-12 h-12" />
                            <div class="flex flex-col gap-1">
                                <span class="font-semibold">
                                    {key.name}
                                </span>
                                <div class="flex flex-row gap-1">
                                    <span class="text-xs text-gray-500">
                                        <Name user={ndk.getUser({ pubkey: key.public_key })} />
                                    </span>
                                    <span class="font-mono text-xs text-gray-500">
                                        ({truncatedNpubForPubkey(key.public_key)}&hellip;)
                                    </span>
                                </div>
                            </div>
                        </a>
                    {/each}
                </div>
            {/if}
            <a href={`/teams/${id}/keys/new`} class="button button-primary">Add Key</a>
        </div>
    </PageSection>

    <PageSection title="Policies">
        <div class="flex flex-col gap-4">
            {#if policies.length === 0}
                <p class="text-gray-500">No policies found</p>
            {:else}
                <div class="card-grid">
                    {#each policies as policy}
                        <PolicyCard {policy} />
                    {/each}
                </div>
            {/if}
            <a href={`/teams/${id}/policies/new`} class="button button-primary self-start">Add Policy</a>
        </div>
    </PageSection>

    {#if users && users.some((team_user) => team_user.user_public_key === user?.pubkey && team_user.role === "Admin")}
        <PageSection title="Danger Zone">
            <button onclick={deleteTeam} class="button button-danger">Delete Team</button>
        </PageSection>
    {/if}
{/if}
