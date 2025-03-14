<script lang="ts">
import Loader from "$lib/components/Loader.svelte";
import TeamCard from "$lib/components/TeamCard.svelte";
import { getCurrentUser } from "$lib/current_user.svelte";
import { KeycastApi } from "$lib/keycast_api.svelte";
import ndk from "$lib/ndk.svelte.js";
import type { TeamWithRelations } from "$lib/types";
import { type NDKEvent, NDKNip07Signer } from "@nostr-dev-kit/ndk";
import { PlusCircle } from "phosphor-svelte";
import { toast } from "svelte-hot-french-toast";

const api = new KeycastApi();
const user = $derived(getCurrentUser()?.user);
let isLoading = $state(true);
let unsignedAuthEvent: NDKEvent | null = $state(null);
let encodedAuthEvent: string | null = $state(null);
let teams: TeamWithRelations[] | null = $state(null);
let teamFormVisible = $state(false);
let newTeamName = $state("");
let newTeamError: string | null = $state(null);
let teamNameInput: HTMLInputElement | null = $state(null);

let inlineTeamFormVisible = $state(false);
let inlineTeamNameInput: HTMLInputElement | null = $state(null);
let inlineTeamError: string | null = $state(null);
let inlineTeamName = $state("");

const apiDomain = import.meta.env.VITE_DOMAIN;
console.log(apiDomain);

$effect(() => {
    if (user?.pubkey && !unsignedAuthEvent) {
        api.buildUnsignedAuthEvent("/teams", "GET", user.pubkey).then(
            async (event) => {
                unsignedAuthEvent = event;
                if (unsignedAuthEvent) {
                    if (!ndk.signer) {
                        ndk.signer = new NDKNip07Signer();
                    }
                    await unsignedAuthEvent.sign();
                    encodedAuthEvent = `Nostr ${btoa(JSON.stringify(unsignedAuthEvent))}`;
                    api.get("/teams", {
                        headers: { Authorization: encodedAuthEvent },
                    })
                        .then((teamsResponse) => {
                            teams = teamsResponse as TeamWithRelations[];
                        })
                        .catch((error) => {
                            console.error(error);
                        })
                        .finally(() => {
                            isLoading = false;
                        });
                }
            },
        );
    }
});

function toggleTeamForm() {
    teamFormVisible = !teamFormVisible;
    if (teamFormVisible) {
        setTimeout(() => teamNameInput?.focus(), 0);
    }
}

function toggleInlineTeamForm() {
    inlineTeamFormVisible = !inlineTeamFormVisible;
    if (inlineTeamFormVisible) {
        setTimeout(() => inlineTeamNameInput?.focus(), 0);
    }
}

async function createTeam(inline = false) {
    if (!user?.pubkey) return;

    const name = inline ? inlineTeamName : newTeamName;

    const authEvent = await api.buildUnsignedAuthEvent(
        "/teams",
        "POST",
        user?.pubkey,
        JSON.stringify({ name }),
    );
    if (!ndk.signer) {
        ndk.signer = new NDKNip07Signer();
    }
    await authEvent?.sign();
    api.post<TeamWithRelations>(
        "/teams",
        { name },
        {
            headers: {
                Authorization: `Nostr ${btoa(JSON.stringify(authEvent))}`,
            },
        },
    )
        .then((newTeam) => {
            teams?.push(newTeam);
            newTeamName = "";
            inlineTeamName = "";
            if (inline) {
                toggleInlineTeamForm();
            } else {
                toggleTeamForm();
            }
            toast.success("Team created successfully");
        })
        .catch((error) => {
            toast.error(`Failed to create team: ${error.message}`);
            if (inline) {
                inlineTeamError = error.message;
            } else {
                newTeamError = error.message;
            }
        });
}
</script>

<div class="flex flex-col md:flex-row items-center justify-between mb-4">
    <h1 class="page-header mb-0! self-start md:self-center">Teams</h1>
    {#if inlineTeamFormVisible}
        <form onsubmit={() => createTeam(true)} class="self-end md:self-center">
            <div class="flex flex-row gap-2">
                <input bind:this={inlineTeamNameInput} type="text" placeholder="Team name" bind:value={inlineTeamName} />
                <button type="submit" class="button button-primary">
                    Create
                </button>
                <button onclick={toggleInlineTeamForm} class="button button-secondary">
                    Cancel
                </button>
            </div>
            {#if inlineTeamError}
                <span class="input-error">{inlineTeamError}</span>
            {/if}
        </form>
    {:else}
        <button onclick={toggleInlineTeamForm} class="button button-primary button-icon self-end md:self-center">
            <PlusCircle size="20" />
            Create a team
        </button>
    {/if}
</div>
{#if isLoading}
    <Loader />
{:else if teams && teams.length > 0}
    <div class="card-grid">
        {#each teams as team}
            <TeamCard team={team} />
        {/each}
    </div>
{:else}
    <div class="flex flex-col items-center justify-center gap-4">
        <p>You don't have any teams yet.</p>
        <button onclick={toggleTeamForm} class="button button-primary button-icon">
            <PlusCircle size="20" />
            Create a team
        </button>
        {#if teamFormVisible}
            <form onsubmit={() => createTeam()}>
                <div class="flex flex-row gap-2">
                    <input bind:this={teamNameInput} type="text" placeholder="Team name" bind:value={newTeamName} />
                    <button type="submit" class="button button-primary">
                        Create
                    </button>
                </div>
                {#if newTeamError}
                    <span class="input-error">{newTeamError}</span>
                {/if}
            </form>
        {/if}
    </div>
{/if}
