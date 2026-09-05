<script lang="ts">
    import { onMount } from "svelte";
    import { getCurrentUser } from "$lib/current_user.svelte";
    import { KeycastApi } from "$lib/keycast_api.svelte";
    import type {
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
    import { Key, CaretDown, ArrowClockwise, Plus } from "phosphor-svelte";
    import {
        formattedUnixDateTime,
        formattedDate,
        dateFromUnixSeconds,
    } from "$lib/utils/dates";
    import { toast } from "svelte-hot-french-toast";

    let {
        id,
        initialTeam,
        onChanged,
        onDeleted,
    }: {
        id: string;
        initialTeam?: TeamWithRelations;
        onChanged?: (team: TeamWithRelations) => void;
        onDeleted?: () => void;
    } = $props();
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
        <div class="stat">
            <strong
                >{team.stored_keys.length.toString().padStart(2, "0")}</strong
            ><span>Managed keys</span>
        </div>
        <div class="stat">
            <strong>{team.policies.length.toString().padStart(2, "0")}</strong
            ><span>Access policies</span>
        </div>
        <div class="stat">
            <strong>{team.team_users.length.toString().padStart(2, "0")}</strong
            ><span>Members</span>
        </div>
    </div>
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
                Your keys will live here. Import a key, define a policy, then
                connect your first app.
            </div>{:else}
            <div class="border-t border-line">
                {#each team.stored_keys as key (key.public_key)}
                    {#snippet keySummary()}
                        <span class="key-symbol"><Key size={18} /></span><span
                            class="min-w-0"
                            ><span class="block text-sm font-medium truncate"
                                >{key.name}</span
                            ><span
                                class="block font-mono text-[11px] text-muted mt-1 truncate"
                                >{key.public_key.slice(
                                    0,
                                    16,
                                )}…{key.public_key.slice(-8)}</span
                            ></span
                        ><span class="key-date text-xs text-muted"
                            >{formattedDate(
                                dateFromUnixSeconds(key.created_at),
                            )}</span
                        >
                    {/snippet}
                    {#if admin}<button
                            class="key-row"
                            class:selected={expandedKey === key.public_key}
                            aria-expanded={expandedKey === key.public_key}
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
    <div class="workspace-grid">
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
                    Define what connected apps can do. Any capability you do not
                    allow is denied.
                </div>{/if}
            {#each team.policies as policy (policy.id)}<div class="policy-row">
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
                    <span class="member-initial"
                        >{member.user_public_key
                            .slice(0, 2)
                            .toUpperCase()}</span
                    >
                    <div class="min-w-0 grow">
                        <p
                            class="font-mono text-[11px] truncate"
                            title={member.user_public_key}
                        >
                            {member.user_public_key.slice(
                                0,
                                12,
                            )}…{member.user_public_key.slice(
                                -6,
                            )}{member.user_public_key === user?.pubkey
                                ? " · you"
                                : ""}
                        </p>
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
                            class="button button-quiet"
                            disabled={busy}
                            aria-label={`Remove member ${member.user_public_key}`}
                            onclick={() => removeMember(member)}>Remove</button
                        >{/if}
                </div>
            {/each}
        </section>
    </div>
    <section class="section" aria-label="Recent activity">
        <div class="section-heading">
            <h2>Recent activity</h2>
            <span class="eyebrow"
                >{auditLoading ? "Loading…" : "Latest signed actions"}</span
            >
        </div>
        {#if auditError}<p class="input-error" role="alert">
                {auditError}
            </p>{:else if !auditLoading && !events.length}<div
                class="empty-state"
            >
                No activity yet. Signed management and signing actions will
                appear here.
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
                        >{#each events.slice(0, 12) as event (event.id)}<tr
                                ><td class="font-mono">{event.action}</td><td
                                    ><span
                                        class="badge"
                                        class:badge-danger={event.outcome ===
                                            "denied" ||
                                            event.outcome === "failed"}
                                        >{event.outcome}</span
                                    ></td
                                ><td class="text-muted font-mono text-[11px]"
                                    >{event.actor_public_key
                                        ? `${event.actor_public_key.slice(0, 10)}…`
                                        : "system"}{event.reason_code
                                        ? ` / ${event.reason_code}`
                                        : ""}</td
                                ><td class="text-muted whitespace-nowrap"
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
    {#if admin}<details class="mt-8 border-t border-line pt-4">
            <summary class="text-xs text-muted">Team settings</summary>
            <p class="description mt-4 mb-3">
                Deleting this team removes its keys and revokes all related
                access.
            </p>
            <button
                class="button button-danger"
                disabled={busy}
                onclick={() =>
                    remove(
                        `/teams/${id}`,
                        "Permanently delete this team, its keys, and all attached access?",
                        () => onDeleted?.(),
                    )}>Delete team</button
            >
        </details>{/if}
{/if}
