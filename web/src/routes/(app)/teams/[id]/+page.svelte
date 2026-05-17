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
import type {
    PolicyWithPermissions,
    StoredKey,
    TeamWithRelations,
    User,
} from "$lib/types";
import { truncatedNpubForPubkey } from "$lib/utils/nostr";
import { DotsThreeVertical } from "phosphor-svelte";
import { toast } from "svelte-hot-french-toast";

const { id } = $page.params;

const api = new KeycastApi();
const user = $derived(getCurrentUser()?.user);
let isLoading = $state(true);
let teamAuthHeader: string | null = $state(null);
let team: TeamWithRelations | null = $state(null);
let users: User[] = $state([]);
let storedKeys: StoredKey[] = $state([]);
let policies: PolicyWithPermissions[] = $state([]);
let isAdmin = $derived(
    users.some(
        (team_user) =>
            team_user.user_public_key === user?.pubkey &&
            team_user.role === "Admin",
    ),
);

$effect(() => {
    if (user?.pubkey && !teamAuthHeader) {
        api.buildAuthHeader(`/teams/${id}`, "GET", user.pubkey)
            .then((authHeader) => {
                teamAuthHeader = authHeader;
                return api.get(`/teams/${id}`, {
                    headers: { Authorization: authHeader },
                });
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
});

async function deleteTeam() {
    if (!user?.pubkey) return;
    if (
        confirm(
            "Are you sure you want to delete this team? This action is irreversible.",
        )
    ) {
        const authHeader = await api.buildAuthHeader(
            `/teams/${id}`,
            "DELETE",
            user?.pubkey,
        );

        api.delete(`/teams/${id}`, {
            headers: {
                Authorization: authHeader,
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

    const authHeader = await api.buildAuthHeader(
        `/teams/${id}/users/${userToRemove.user_public_key}`,
        "DELETE",
        user?.pubkey,
    );

    api.delete(`/teams/${id}/users/${userToRemove.user_public_key}`, {
        headers: {
            Authorization: authHeader,
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
                <div class="card flex flex-row! gap-4 relative">
                    <Avatar pubkey={user.user_public_key} extraClasses="w-12 h-12" />
                    <div class="flex flex-col gap-1">
                        <span class="font-semibold">
                            <Name pubkey={user.user_public_key} />
                        </span>
                        <span class="font-mono text-xs text-gray-500">
                            {truncatedNpubForPubkey(user.user_public_key)}&hellip;
                        </span>
                    </div>
                    <AdminPill {user} />
                    {#if isAdmin}
                        <button onclick={() => showUserMenu(user)} class="absolute top-1.5 right-1"><DotsThreeVertical size={20} weight="bold" class="text-gray-500 hover:text-gray-200" /></button>
                        <div id={`user-menu-${user.user_public_key}`} class="hidden absolute top-8 right-1 bg-gray-700 ring-1 ring-gray-600 shadow-lg rounded-md p-2 text-sm">
                            <button onclick={() => removeUser(user)} class="text-gray-200 hover:text-white">Remove User</button>
                        </div>
                    {/if}
                </div>
            {/each}
        </div>
        {#if isAdmin}
            <a href={`/teams/${id}/users/new`} class="button button-primary">Add Member</a>
        {/if}
    </PageSection>


    <PageSection title="Keys">
        <div class="flex flex-col gap-4 items-start">
            {#if storedKeys.length === 0}
                <p class="text-gray-500">No keys found</p>
            {:else}
                <div class="card-grid">
                    {#each storedKeys as key}
                        {#if isAdmin}
                            <a href={`/teams/${id}/keys/${key.public_key}`} class="card hover-card flex flex-row! gap-4 ">
                                <Avatar pubkey={key.public_key} extraClasses="w-12 h-12" />
                                <div class="flex flex-col gap-1">
                                    <span class="font-semibold">
                                        {key.name}
                                    </span>
                                    <div class="flex flex-row gap-1">
                                        <span class="text-xs text-gray-500">
                                            <Name pubkey={key.public_key} />
                                        </span>
                                        <span class="font-mono text-xs text-gray-500">
                                            ({truncatedNpubForPubkey(key.public_key)}&hellip;)
                                        </span>
                                    </div>
                                </div>
                            </a>
                        {:else}
                            <div class="card flex flex-row! gap-4 ">
                                <Avatar pubkey={key.public_key} extraClasses="w-12 h-12" />
                                <div class="flex flex-col gap-1">
                                    <span class="font-semibold">
                                        {key.name}
                                    </span>
                                    <div class="flex flex-row gap-1">
                                        <span class="text-xs text-gray-500">
                                            <Name pubkey={key.public_key} />
                                        </span>
                                        <span class="font-mono text-xs text-gray-500">
                                            ({truncatedNpubForPubkey(key.public_key)}&hellip;)
                                        </span>
                                    </div>
                                </div>
                            </div>
                        {/if}
                    {/each}
                </div>
            {/if}
            {#if isAdmin}
                <a href={`/teams/${id}/keys/new`} class="button button-primary">Add Key</a>
            {/if}
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
            {#if isAdmin}
                <a href={`/teams/${id}/policies/new`} class="button button-primary self-start">Add Policy</a>
            {/if}
        </div>
    </PageSection>

    {#if isAdmin}
        <PageSection title="Danger Zone">
            <button onclick={deleteTeam} class="button button-danger">Delete Team</button>
        </PageSection>
    {/if}
{/if}
