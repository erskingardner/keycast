<script lang="ts">
    import {
        teamSectionPath,
        type WorkspaceSection,
    } from "$lib/utils/team_url";
    import { onMount } from "svelte";
    import UserIdentity from "./UserIdentity.svelte";
    import { getCurrentUser } from "$lib/current_user.svelte";
    import { KeycastApi } from "$lib/keycast_api.svelte";
    import type {
        Team,
        TeamWithRelations,
        AuditEvent,
        Policy,
        User,
    } from "$lib/types";
    import KeyPanel from "./KeyPanel.svelte";
    import KeyImport from "./KeyImport.svelte";
    import MemberEditor from "./MemberEditor.svelte";
    import PolicyEditor from "./PolicyEditor.svelte";
    import PolicyCard from "./PolicyCard.svelte";
    import Loader from "./Loader.svelte";
    import {
        CaretDown,
        ArrowClockwise,
        Plus,
        Key,
        ShieldCheck,
        Users,
        Pulse,
        GearSix,
    } from "phosphor-svelte";
    import {
        formattedUnixDateTime,
        formattedDate,
        dateFromUnixSeconds,
    } from "$lib/utils/dates";
    import { toast } from "svelte-hot-french-toast";

    let {
        id,
        initialTeam,
        section = "keys",
        onChanged,
        onDeleted,
    }: {
        id: string;
        initialTeam?: TeamWithRelations;
        section?: WorkspaceSection;
        onChanged?: (team: TeamWithRelations) => void;
        onDeleted?: () => void;
    } = $props();
    const sections = [
        { id: "keys", label: "Keys", icon: Key },
        { id: "policies", label: "Policies", icon: ShieldCheck },
        { id: "members", label: "Members", icon: Users },
        { id: "activity", label: "Activity", icon: Pulse },
        { id: "settings", label: "Settings", icon: GearSix },
    ] as const;
    const api = new KeycastApi();
    const user = $derived(getCurrentUser()?.user);
    let team = $state<TeamWithRelations | null>(null);
    let loading = $state(true);
    let error = $state("");
    let auditError = $state("");
    let auditLoading = $state(true);
    let events = $state<AuditEvent[]>([]);
    let form = $state<"key" | "member" | "policy" | null>(null);
    let editingPolicy = $state<Policy | undefined>();
    let expandedKey = $state<string | null>(null);
    let busy = $state(false);
    let teamName = $state("");
    let nameError = $state("");
    let keyRevision = $state(0);
    let disposed = false;
    const admin = $derived(
        team?.team_users.some(
            (member) =>
                member.user_public_key === user?.pubkey &&
                member.role === "admin",
        ) ?? false,
    );
    onMount(() => {
        if (initialTeam) {
            team = initialTeam;
            teamName = initialTeam.team.name;
            loading = false;
            void loadAudit();
        } else void refresh();
        return () => {
            disposed = true;
        };
    });
    async function read<T>(endpoint: string): Promise<T> {
        if (!user) throw new Error("Sign in to load this workspace.");
        const authorization = await api.buildAuthHeader(
            endpoint,
            "GET",
            user.pubkey,
        );
        return api.get<T>(endpoint, {
            headers: { Authorization: authorization },
        });
    }
    async function loadAudit() {
        auditLoading = true;
        auditError = "";
        try {
            events = await read<AuditEvent[]>(`/teams/${id}/audit`);
        } catch (e) {
            auditError =
                e instanceof Error ? e.message : "Activity could not be loaded";
        } finally {
            auditLoading = false;
        }
    }
    async function refresh() {
        loading = true;
        error = "";
        try {
            const loaded = await read<TeamWithRelations>(`/teams/${id}`);
            if (disposed) return;
            team = loaded;
            teamName = loaded.team.name;
            keyRevision += 1;
            onChanged?.(team);
            await loadAudit();
        } catch (e) {
            error =
                e instanceof Error
                    ? e.message
                    : "Workspace could not be loaded";
        } finally {
            loading = false;
        }
    }
    async function renameTeam() {
        if (!user || !team || !admin || busy || loading) return;
        const name = teamName.trim();
        nameError = "";
        if (!name || [...name].length > 120) {
            nameError = "Use between 1 and 120 characters.";
            return;
        }
        if (name === team.team.name) return;
        busy = true;
        try {
            const endpoint = `/teams/${id}`;
            const request = { name };
            const authorization = await api.buildAuthHeader(
                endpoint, "PUT", user.pubkey, JSON.stringify(request),
            );
            const updated = await api.put<Team>(endpoint, request, {
                headers: { Authorization: authorization },
            });
            if (disposed) return;
            team = { ...team, team: updated };
            teamName = updated.name;
            onChanged?.(team);
            toast.success("Team name updated");
            void loadAudit();
        } catch (e) {
            if (!disposed) nameError = e instanceof Error ? e.message : "Could not update the team name.";
        } finally {
            busy = false;
        }
    }

    async function saved() {
        form = null;
        editingPolicy = undefined;
        await refresh();
    }
    function toggleForm(next: "key" | "member" | "policy") {
        form = form === next ? null : next;
        editingPolicy = undefined;
    }
    async function remove(
        endpoint: string,
        question: string,
        success: () => void | Promise<void>,
    ) {
        if (!user || busy || !confirm(question)) return;
        busy = true;
        try {
            const authorization = await api.buildAuthHeader(
                endpoint,
                "DELETE",
                user.pubkey,
            );
            await api.delete(endpoint, {
                headers: { Authorization: authorization },
            });
            await success();
        } catch (e) {
            toast.error(
                e instanceof Error
                    ? e.message
                    : "Could not complete this action",
            );
        } finally {
            busy = false;
        }
    }
    async function removeMember(member: User) {
        await remove(
            `/teams/${id}/users/${member.user_public_key}`,
            "Remove this member from the team?",
            refresh,
        );
    }
    async function removePolicy(policy: Policy) {
        await remove(
            `/teams/${id}/policies/${policy.id}`,
            `Delete policy “${policy.name}”?`,
            refresh,
        );
    }
</script>

{#if loading && !team}<Loader />{:else if !team}<div class="empty-state">
        <p role="alert" class="input-error">{error}</p>
        <button class="button button-secondary mt-3" onclick={refresh}
            >Retry</button
        >
    </div>{:else}
    <div class="flex items-start justify-between gap-3">
        <div>
            <p class="eyebrow mb-2">Team workspace</p>
            <h1 class="page-header">{team.team.name}</h1>
            <p class="description">
                Manage keys, app access, and the people behind them.
            </p>
        </div>
        <button
            class="button button-secondary"
            onclick={refresh}
            disabled={loading || busy}
            aria-label="Refresh workspace"
            ><ArrowClockwise size={16} /><span class="hidden sm:inline"
                >Refresh</span
            ></button
        >
    </div>
    {#if error}<p role="alert" class="input-error mt-3">{error}</p>{/if}
    <div class="stat-strip">
        <a class="stat" href={teamSectionPath(team.team, "keys")}>
            <strong
                >{team.stored_keys.length.toString().padStart(2, "0")}</strong
            ><span>Managed keys</span>
        </a>
        <a class="stat" href={teamSectionPath(team.team, "policies")}>
            <strong>{team.policies.length.toString().padStart(2, "0")}</strong
            ><span>Access policies</span>
        </a>
        <a class="stat" href={teamSectionPath(team.team, "members")}>
            <strong>{team.team_users.length.toString().padStart(2, "0")}</strong
            ><span>Members</span>
        </a>
    </div>
    <div class="workspace-sections">
        <nav class="section-rail" aria-label="Team sections">
            <p class="eyebrow section-rail-label">Explore</p>
            {#each sections as item}
                <a
                    class="section-choice"
                    class:active={section === item.id}
                    href={teamSectionPath(team.team, item.id)}
                    aria-current={section === item.id ? "page" : undefined}
                >
                    <item.icon size={16} weight="regular" />
                    <span>{item.label}</span>
                    {#if item.id === "keys"}<small
                            >{team.stored_keys.length}</small
                        >
                    {:else if item.id === "policies"}<small
                            >{team.policies.length}</small
                        >
                    {:else if item.id === "members"}<small
                            >{team.team_users.length}</small
                        >{/if}
                </a>
            {/each}
        </nav>
        <div class="workspace-content">
            {#if section === "keys"}
                <section class="section" aria-label="Managed keys">
                    <div class="section-heading">
                        <h2>Keys <span>{team.stored_keys.length}</span></h2>
                        {#if admin}<button
                                class="button button-primary"
                                aria-expanded={form === "key"}
                                onclick={() => toggleForm("key")}
                                ><Plus size={14} />{form === "key"
                                    ? "Close import"
                                    : "Import key"}</button
                            >{/if}
                    </div>
                    {#if form === "key"}<div class="form-panel">
                            <KeyImport {id} onSaved={saved} />
                        </div>{/if}
                    {#if !team.stored_keys.length}<div class="empty-state">
                            Your keys will live here. Import a key, define a
                            policy, then connect your first app.
                        </div>{:else}
                        <div class="border-t border-line">
                            {#each team.stored_keys as key (key.public_key)}
                                {#snippet keySummary()}
                                    <span class="col-span-2 min-w-0">
                                        <span
                                            class="block text-sm font-medium truncate mb-2"
                                            >{key.name}</span
                                        >
                                        <UserIdentity pubkey={key.public_key} />
                                    </span><span
                                        class="key-date text-xs text-muted"
                                        >{formattedDate(
                                            dateFromUnixSeconds(key.created_at),
                                        )}</span
                                    >
                                {/snippet}
                                {#if admin}<button
                                        class="key-row"
                                        class:selected={expandedKey ===
                                            key.public_key}
                                        aria-expanded={expandedKey ===
                                            key.public_key}
                                        onclick={() =>
                                            (expandedKey =
                                                expandedKey === key.public_key
                                                    ? null
                                                    : key.public_key)}
                                        >{@render keySummary()}<CaretDown
                                            size={14}
                                        /></button
                                    >{:else}<div class="key-row">
                                        {@render keySummary()}
                                    </div>{/if}
                                {#if admin && expandedKey === key.public_key}<div
                                        class="key-detail"
                                    >
                                        {#key keyRevision}<KeyPanel
                                                {id}
                                                pubkey={key.public_key}
                                                onRemoved={async () => {
                                                    expandedKey = null;
                                                    await refresh();
                                                }}
                                            />{/key}
                                    </div>{/if}
                            {/each}
                        </div>
                    {/if}
                </section>
            {:else if section === "policies"}
                <section class="section" aria-label="Access policies">
                    <div class="section-heading">
                        <h2>Policies <span>{team.policies.length}</span></h2>
                        {#if admin}<button
                                class="button button-secondary"
                                aria-expanded={form === "policy"}
                                onclick={() => toggleForm("policy")}
                                >{form === "policy"
                                    ? "Close editor"
                                    : "New policy"}</button
                            >{/if}
                    </div>
                    {#if form === "policy"}<div class="form-panel">
                            {#key editingPolicy?.id}<PolicyEditor
                                    {id}
                                    policy={editingPolicy}
                                    onSaved={saved}
                                />{/key}
                        </div>{/if}
                    {#if !team.policies.length}<div class="empty-state">
                            Define what connected apps can do. Any capability
                            you do not allow is denied.
                        </div>{/if}
                    {#each team.policies as policy (policy.id)}<div
                            class="policy-row"
                        >
                            <PolicyCard
                                {policy}
                                onEdit={admin
                                    ? (item) => {
                                          editingPolicy = item;
                                          form = "policy";
                                      }
                                    : undefined}
                                onRemove={admin ? removePolicy : undefined}
                            />
                        </div>{/each}
                </section>
            {:else if section === "members"}
                <section class="section" aria-label="Team members">
                    <div class="section-heading">
                        <h2>Members <span>{team.team_users.length}</span></h2>
                        {#if admin}<button
                                class="button button-secondary"
                                aria-expanded={form === "member"}
                                onclick={() => toggleForm("member")}
                                >{form === "member"
                                    ? "Close form"
                                    : "Add member"}</button
                            >{/if}
                    </div>
                    {#if form === "member"}<div class="form-panel">
                            <MemberEditor {id} onSaved={saved} />
                        </div>{/if}
                    {#each team.team_users as member (member.user_public_key)}
                        <div class="member-row">
                            <div class="min-w-0 grow">
                                <UserIdentity
                                    pubkey={member.user_public_key}
                                    isYou={member.user_public_key ===
                                        user?.pubkey}
                                />
                                <p class="text-muted text-[11px] mt-1">
                                    Joined {formattedDate(
                                        dateFromUnixSeconds(member.created_at),
                                    )}
                                </p>
                            </div>
                            <span
                                class="badge"
                                class:badge-neutral={member.role !== "admin"}
                                >{member.role}</span
                            >{#if admin}<button
                                    class="button button-danger"
                                    disabled={busy}
                                    aria-label={`Remove member ${member.user_public_key}`}
                                    onclick={() => removeMember(member)}
                                    >Remove</button
                                >{/if}
                        </div>
                    {/each}
                </section>
            {:else if section === "activity"}
                <section class="section" aria-label="Recent activity">
                    <div class="section-heading">
                        <h2>Recent activity</h2>
                        <span class="eyebrow"
                            >{auditLoading
                                ? "Loading…"
                                : "Latest signed actions"}</span
                        >
                    </div>
                    {#if auditError}<p class="input-error" role="alert">
                            {auditError}
                        </p>{:else if !auditLoading && !events.length}<div
                            class="empty-state"
                        >
                            No activity yet. Signed management and signing
                            actions will appear here.
                        </div>{:else}
                        <div class="table-scroll">
                            <table class="data-table">
                                <thead
                                    ><tr
                                        ><th>Action</th><th>Outcome</th><th
                                            >Actor / reason</th
                                        ><th>Time</th></tr
                                    ></thead
                                ><tbody
                                    >{#each events as event (event.id)}<tr
                                            ><td class="font-mono"
                                                >{event.action}</td
                                            ><td
                                                ><span
                                                    class="badge"
                                                    class:badge-danger={event.outcome ===
                                                        "denied" ||
                                                        event.outcome ===
                                                            "failed"}
                                                    >{event.outcome}</span
                                                ></td
                                            ><td
                                                class="text-muted font-mono text-[11px]"
                                                >{#if event.actor_public_key}<UserIdentity
                                                        pubkey={event.actor_public_key}
                                                    />{:else}system{/if}
                                                {#if event.reason_code}<span
                                                        class="block mt-1"
                                                        >{event.reason_code}</span
                                                    >{/if}</td
                                            ><td
                                                class="text-muted whitespace-nowrap"
                                                >{formattedUnixDateTime(
                                                    event.occurred_at,
                                                )}</td
                                            ></tr
                                        >{/each}</tbody
                                >
                            </table>
                        </div>
                    {/if}
                </section>
            {:else if section === "settings"}
                <section class="section" aria-label="Team settings">
                    <div class="section-heading"><h2>Settings</h2></div>
                    {#if admin}
                        <form class="border-t border-line pt-5 pb-6" onsubmit={(event) => {
                            event.preventDefault();
                            void renameTeam();
                        }}>
                            <label class="eyebrow" for="workspace-name">Team name</label>
                            <div class="flex flex-wrap items-start gap-3 mt-2 max-w-xl">
                                <input id="workspace-name" class="grow min-w-0 basis-56" type="text" required maxlength="120"
                                    bind:value={teamName} disabled={busy || loading}
                                    aria-invalid={nameError ? true : undefined}
                                    aria-describedby={nameError ? "workspace-name-error" : "workspace-name-help"} />
                                <button class="button button-primary" type="submit"
                                    disabled={busy || loading || !teamName.trim() || teamName.trim() === team.team.name}>
                                    {busy ? "Saving…" : "Save name"}
                                </button>
                            </div>
                            <p id="workspace-name-help" class="description mt-2">Your team's URL stays the same when you rename it.</p>
                            {#if nameError}<p id="workspace-name-error" class="input-error mt-2" role="alert">{nameError}</p>{/if}
                        </form>
                    {/if}
                    <dl class="team-settings-info">
                        {#if !admin}<div>
                            <dt class="eyebrow">Team name</dt>
                            <dd>{team.team.name}</dd>
                        </div>{/if}
                        <div>
                            <dt class="eyebrow">Created</dt>
                            <dd>
                                {formattedDate(
                                    dateFromUnixSeconds(team.team.created_at),
                                )}
                            </dd>
                        </div>
                    </dl>
                    {#if admin}
                        <div class="mt-8 border-t border-line pt-5">
                            <h3 class="text-sm mb-2">Delete team</h3>
                            <p class="description mb-4">
                                Deleting this team removes its keys and revokes
                                all related access.
                            </p>
                            <button
                                class="button button-danger"
                                disabled={busy}
                                onclick={() =>
                                    remove(
                                        `/teams/${id}`,
                                        "Permanently delete this team, its keys, and all attached access?",
                                        () => onDeleted?.(),
                                    )}
                            >
                                Delete team
                            </button>
                        </div>
                    {:else}<p class="description mt-6">
                            Only team administrators can change team settings.
                        </p>{/if}
                </section>
            {/if}
        </div>
    </div>
{/if}
