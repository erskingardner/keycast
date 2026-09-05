<script lang="ts">
    import { getCurrentUser } from "$lib/current_user.svelte";
    import { KeycastApi } from "$lib/keycast_api.svelte";
    import type { TeamWithRelations } from "$lib/types";
    import TeamWorkspace from "$lib/components/TeamWorkspace.svelte";
    import Loader from "$lib/components/Loader.svelte";
    import { Plus, SquaresFour } from "phosphor-svelte";
    const api = new KeycastApi();
    const user = $derived(getCurrentUser()?.user);
    let teams = $state<TeamWithRelations[]>([]);
    let selected = $state<number | null>(null);
    let loading = $state(true);
    let startedFor = $state("");
    let error = $state("");
    let creating = $state(false);
    let busy = $state(false);
    let name = $state("");
    let createError = $state("");
    const activeTeam = $derived(
        teams.find((team) => team.team.id === selected),
    );
    $effect(() => {
        if (user?.pubkey && startedFor !== user.pubkey) {
            startedFor = user.pubkey;
            teams = [];
            selected = null;
            void load();
        }
    });
    async function load() {
        if (!user) return;
        loading = true;
        error = "";
        try {
            const authorization = await api.buildAuthHeader(
                "/teams",
                "GET",
                user.pubkey,
            );
            teams = await api.get<TeamWithRelations[]>("/teams", {
                headers: { Authorization: authorization },
            });
            if (!teams.some((team) => team.team.id === selected))
                selected = teams[0]?.team.id ?? null;
        } catch (e) {
            error = e instanceof Error ? e.message : "Could not load teams";
        } finally {
            loading = false;
        }
    }
    async function create() {
        if (!user || busy) return;
        busy = true;
        createError = "";
        try {
            const request = { name: name.trim() };
            const authorization = await api.buildAuthHeader(
                "/teams",
                "POST",
                user.pubkey,
                JSON.stringify(request),
            );
            const team = await api.post<TeamWithRelations>("/teams", request, {
                headers: { Authorization: authorization },
            });
            teams = [...teams, team];
            selected = team.team.id;
            name = "";
            creating = false;
        } catch (e) {
            createError =
                e instanceof Error ? e.message : "Could not create team";
        } finally {
            busy = false;
        }
    }
</script>

<svelte:head><title>Workspace — Keycast</title></svelte:head>
<div class="workspace">
    <aside class="workspace-rail" aria-label="Teams">
        <div class="flex items-center justify-between mb-4">
            <span class="eyebrow">Your teams</span><button
                class="button button-quiet"
                aria-label="Create team"
                aria-expanded={creating}
                onclick={() => (creating = !creating)}
                ><Plus size={16} /></button
            >
        </div>
        {#if creating}<form
                class="mb-5"
                onsubmit={(event) => {
                    event.preventDefault();
                    void create();
                }}
            >
                <label class="eyebrow" for="team-name">Team name</label><input
                    id="team-name"
                    type="text"
                    required
                    maxlength="120"
                    bind:value={name}
                    placeholder="My workspace"
                    class="mt-2"
                />{#if createError}<p role="alert" class="input-error mt-2">
                        {createError}
                    </p>{/if}
                <div class="form-actions">
                    <button class="button button-primary" disabled={busy}
                        >{busy ? "Creating…" : "Create"}</button
                    ><button
                        type="button"
                        class="button button-quiet"
                        onclick={() => (creating = false)}>Cancel</button
                    >
                </div>
            </form>{/if}
        <div class="team-list">
            {#each teams as team (team.team.id)}<button
                    class="team-choice"
                    class:active={selected === team.team.id}
                    aria-current={selected === team.team.id
                        ? "true"
                        : undefined}
                    onclick={() => (selected = team.team.id)}
                    ><SquaresFour size={16} class="mt-0.5 shrink-0" /><span
                        class="min-w-0"
                        ><span class="block truncate">{team.team.name}</span
                        ><small
                            >{team.stored_keys.length}
                            {team.stored_keys.length === 1 ? "key" : "keys"} · {team
                                .team_users.length}
                            {team.team_users.length === 1
                                ? "member"
                                : "members"}</small
                        ></span
                    ></button
                >{/each}
        </div>
        <p class="rail-note description mt-8 border-t border-line pt-4">
            Each team has its own keys, policies, and members.
        </p>
    </aside>
    <div class="min-w-0">
        {#if loading}<Loader />{:else if error}<div class="empty-state">
                <p class="input-error" role="alert">{error}</p>
                <button class="button button-secondary mt-3" onclick={load}
                    >Retry</button
                >
            </div>{:else if activeTeam}
            {#key `${user?.pubkey}:${selected}`}<TeamWorkspace
                    id={String(activeTeam.team.id)}
                    initialTeam={activeTeam}
                    onChanged={(updated) =>
                        (teams = teams.map((team) =>
                            team.team.id === updated.team.id ? updated : team,
                        ))}
                    onDeleted={load}
                />{/key}
        {:else}<p class="eyebrow mb-3">Your workspace starts here</p>
            <h1 class="page-header">A home for your keys.</h1>
            <p class="description max-w-lg mt-4">
                Create your first team to organize keys and define app access. A
                team can be just you, or the people you work with.
            </p>
            <button
                class="button button-primary mt-6"
                onclick={() => (creating = true)}
                ><Plus size={16} />Create a team</button
            >{/if}
    </div>
</div>
