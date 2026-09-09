<script lang="ts">
    import RelayHealthRow from "$lib/components/RelayHealthRow.svelte";
    import Loader from "$lib/components/Loader.svelte";
    import { getCurrentUser } from "$lib/current_user.svelte";
    import { KeycastApi } from "$lib/keycast_api.svelte";
    import type { StatusResponse } from "$lib/types";
    import { formattedUnixDateTime } from "$lib/utils/dates";
    import {
        ArrowClockwise,
        CheckCircle,
        WarningCircle,
    } from "phosphor-svelte";
    import { toast } from "svelte-hot-french-toast";
    import {
        isManagementReplyKey,
        managementReplyFingerprint,
        pinnedManagementReplyKey,
        trustManagementReplyKey,
    } from "$lib/utils/reply_identity";

    const api = new KeycastApi();
    const user = $derived(getCurrentUser()?.user);
    let status: StatusResponse | null = $state(null);
    let isLoading = $state(true);
    let isSaving = $state(false);
    let loadError = $state<string | null>(null);
    let minimumConnectedRelays = $state(1);
    let relayLines = $state("");
    let autoActivate = $state(false);
    let pinnedReplyKey = $state<string | null>(null);
    let publishedReplyKey = $state<string | null>(null);
    let replyIdentityError = $state<string | null>(null);
    // A mismatch means either a deliberate root rotation or a substituted key.
    const replyKeyChanged = $derived(
        !!publishedReplyKey &&
            !!pinnedReplyKey &&
            publishedReplyKey !== pinnedReplyKey,
    );

    $effect(() => {
        pinnedReplyKey = pinnedManagementReplyKey();
    });

    /**
     * `/config` is unauthenticated, unlike `/status`, which requires an instance
     * operator. A team administrator who may perform management writes but is not
     * an operator would otherwise be blocked by a rotated identity with no way to
     * see or re-trust the new one.
     */
    async function loadReplyIdentity() {
        if (!user?.pubkey) return;
        replyIdentityError = null;
        try {
            const config = await api.get<{
                management_reply_public_key?: unknown;
            }>("/config", { params: { pubkey: user.pubkey } });
            publishedReplyKey = isManagementReplyKey(
                config.management_reply_public_key,
            )
                ? config.management_reply_public_key
                : null;
            if (!publishedReplyKey) {
                replyIdentityError =
                    "The signer did not publish a management reply identity.";
            }
        } catch (error) {
            publishedReplyKey = null;
            replyIdentityError =
                error instanceof Error
                    ? error.message
                    : "Could not load the management reply identity";
        }
    }

    function trustReplyKey() {
        if (!publishedReplyKey) return;
        trustManagementReplyKey(publishedReplyKey);
        pinnedReplyKey = publishedReplyKey;
        toast.success("Management reply identity trusted");
    }

    async function refreshAll() {
        await Promise.all([refresh(), loadReplyIdentity()]);
    }

    $effect(() => {
        if (user?.pubkey) {
            void refresh();
            void loadReplyIdentity();
        }
    });

    async function refresh() {
        if (!user?.pubkey) return;
        isLoading = true;
        loadError = null;
        try {
            const authorization = await api.buildAuthHeader(
                "/status",
                "GET",
                user.pubkey,
            );
            status = await api.get<StatusResponse>("/status", {
                headers: { Authorization: authorization },
            });
            minimumConnectedRelays = status.minimum_connected_relays;
            autoActivate = status.auto_activate_relays;
            relayLines = status.relays
                .filter((relay) => relay.enabled && !relay.discovered)
                .map((relay) => relay.url)
                .join("\n");
        } catch (error) {
            loadError =
                error instanceof Error
                    ? error.message
                    : "Could not load status";
            toast.error(loadError);
        } finally {
            isLoading = false;
        }
    }

    async function disableDiscovered(url: string) {
        if (!user?.pubkey || !status) return;
        isSaving = true;
        try {
            const body = { minimum_connected_relays: status.minimum_connected_relays,
                relays: [...status.relays.filter(r => !r.discovered).map(r => ({url:r.url, enabled:r.enabled})), {url, enabled:false}] };
            const authorization = await api.buildAuthHeader("/relays", "PUT", user.pubkey, JSON.stringify(body));
            await api.put("/relays", body, {headers:{Authorization:authorization}});
            toast.success("Discovered relay disabled"); await refresh();
        } catch(error) { toast.error(error instanceof Error ? error.message : String(error)); }
        finally { isSaving=false; }
    }
    async function saveDiscovery() {
        if (!user?.pubkey) return;
        isSaving = true;
        try {
            const body = { auto_activate: autoActivate };
            const authorization = await api.buildAuthHeader("/relay-discovery", "PUT", user.pubkey, JSON.stringify(body));
            await api.put("/relay-discovery", body, { headers: { Authorization: authorization } });
            toast.success("Relay discovery policy saved");
            await refresh();
        } catch (error) {
            toast.error(error instanceof Error ? error.message : String(error));
        } finally { isSaving = false; }
    }
    async function saveRelays() {
        if (!user?.pubkey) return;
        const relays = relayLines
            .split("\n")
            .map((url) => url.trim())
            .filter(Boolean)
            .map((url) => ({ url, enabled: true }));
        const body = {
            minimum_connected_relays: minimumConnectedRelays,
            relays: [...relays, ...(status?.relays ?? []).filter(r => !r.discovered && !r.enabled && !relays.some(v => v.url === r.url)).map(r => ({url:r.url,enabled:false}))],
        };
        isSaving = true;
        try {
            const authorization = await api.buildAuthHeader(
                "/relays",
                "PUT",
                user.pubkey,
                JSON.stringify(body),
            );
            await api.put("/relays", body, {
                headers: { Authorization: authorization },
            });
            toast.success("Relay configuration saved");
            await refresh();
        } catch (error) {
            toast.error(
                error instanceof Error
                    ? error.message
                    : "Could not save relays",
            );
        } finally {
            isSaving = false;
        }
    }
</script>

<div class="flex flex-row items-center justify-between mb-6">
    <div>
        <h1 class="page-header mb-1!">Instance status</h1>
        <p class="text-muted">
            Signer, database, relay health, and configuration. Relay changes
            require an instance operator and approval from an external signer.
        </p>
    </div>
    <button
        class="button button-secondary button-icon"
        onclick={refreshAll}
        disabled={isLoading}
    >
        <ArrowClockwise size="20" /> Refresh
    </button>
</div>

{#if loadError}<p class="input-error">
        {loadError}. Instance status requires an operator.
    </p>{/if}

<div class="card mb-5" class:border-warning={replyKeyChanged}>
    <h2 class="text-xl font-bold mb-2">Management reply identity</h2>
    {#if replyIdentityError}
        <p class="input-error">{replyIdentityError}</p>
    {:else}
        <p class="description">
            Published by the signer: <code
                >{publishedReplyKey
                    ? managementReplyFingerprint(publishedReplyKey)
                    : "loading"}</code
            >
        </p>
        <p class="description">
            Trusted by this browser: <code
                >{pinnedReplyKey
                    ? managementReplyFingerprint(pinnedReplyKey)
                    : "nothing pinned yet"}</code
            >
        </p>
    {/if}
    {#if replyKeyChanged}
        <p class="text-warning mt-3" role="alert">
            This identity changed, so management writes are blocked. Run
            <code>keycast_signer status</code> on your host and compare its
            <code>management_reply_public_key</code>. Trust the new identity only
            if you rotated the root credential yourself.
        </p>
        <button class="button button-danger mt-3" onclick={trustReplyKey}
            >Trust this identity</button
        >
    {/if}
</div>

{#if isLoading && !status}
    <Loader />
{:else if status}
    <div class="grid md:grid-cols-3 gap-3 mb-5">
        <div class="card">
            <div class="flex items-center gap-2 mb-2">
                {#if status.signer.ready}<CheckCircle
                        class="text-accent"
                        size="22"
                    />{:else}<WarningCircle
                        class="text-warning"
                        size="22"
                    />{/if}
                <h2 class="font-bold">Signer</h2>
            </div>
            <p>{status.signer.ready ? "Ready" : "Not ready"}</p>
            {#if status.signer.recovery_pending}<p class="text-warning">
                    Signing paused for restore review.
                </p>{/if}
            {#if status.signer.quarantined_grants > 0}<p class="text-warning">
                    {status.signer.quarantined_grants} grants need repair; other
                    grants continue.
                </p>{/if}
            <p class="text-sm text-muted">
                {status.signer.connected_relays} of {status.signer
                    .enabled_relays} relays connected
            </p>
        </div>
        <div class="card">
            <h2 class="font-bold mb-2">Database</h2>
            <p>
                {status.database_ok
                    ? "Last integrity check passed"
                    : "Integrity check failed"}
            </p>
            <p class="text-sm text-muted">
                {(status.signer.resources.database_bytes / 1048576).toFixed(1)} MiB
                database · {(
                    status.signer.resources.wal_bytes / 1048576
                ).toFixed(1)} MiB WAL
            </p>
            <p class="text-sm text-muted">
                Last local backup: {formattedUnixDateTime(
                    status.signer.resources.last_backup_at,
                )}
            </p>
            <p class="text-sm text-muted">
                Schema v{status.signer.schema_version}, envelope v{status.signer
                    .envelope_version}
            </p>
        </div>
        <div class="card">
            <h2 class="font-bold mb-2">Activity</h2>
            <p>
                {status.signer.active_grants} active grants · {status.signer
                    .active_sessions} sessions
            </p>
            <p class="text-sm text-muted">
                {status.signer.claimable_invitations} claimable invitations
            </p>
            <p class="text-sm text-muted">
                {status.signer.resources.pending_inputs} queued inputs · {status
                    .signer.resources.pending_responses} pending replies
            </p>
            {#if status.signer.resources.oldest_response_age_seconds > 60}<p
                    class="text-warning"
                >
                    Oldest pending reply: {status.signer.resources
                        .oldest_response_age_seconds}s
                </p>{/if}
            <p class="text-sm text-muted">
                {status.signer.ingress_rejections} requests limited since startup
            </p>
            <p class="text-sm text-muted">
                Last completed request: {formattedUnixDateTime(
                    status.signer.last_processed_at,
                )}
            </p>
        </div>
    </div>

    <div class="card mb-5">
        <h2 class="text-xl font-bold mb-4">Relay health</h2>
        <p class="description mb-2">
            Expand a relay for connection details and recent history.
        </p>
        {#each status.relays as relay (relay.id)}<RelayHealthRow
                {relay}
            />
            {#if relay.discovered && relay.enabled}
                <button class="button button-secondary mb-3" disabled={isSaving} onclick={() => disableDiscovered(relay.url)}>Disable discovered relay</button>
            {/if}
        {/each}
    </div>

    <div class="card mb-5">
        <h2 class="text-xl font-bold mb-3">Diagnostics</h2>
        <div class="grid sm:grid-cols-2 gap-2 text-sm text-muted">
            <p>
                Credential fingerprint: <code
                    >{status.signer.credential_key_id ?? "unavailable"}</code
                >
            </p>
            <p>Denied requests: {status.signer.denied_requests}</p>
            <p>Parse errors: {status.signer.parse_errors}</p>
            <p>Relay failures: {status.signer.relay_failures}</p>
        </div>
    </div>

    <section class="border border-border p-5 mb-6">
        <h2 class="text-xl font-bold mb-3">Relay discovery</h2>
        <label class="flex items-center gap-3 mb-3">
            <input type="checkbox" bind:checked={autoActivate} />
            Activate compatible public relays from imported keys’ NIP-65 lists
        </label>
        <p class="text-muted text-sm mb-4">Lists are cached and refreshed in the background. Each key can add up to four signing relays, within the instance limit of twenty. Relays must pass a signing-transport check. Turning this off stops new activation; existing routes stay available for connected clients.</p>
        <button class="button button-secondary" onclick={saveDiscovery} disabled={isSaving}>Save discovery policy</button>
        <p class="text-muted text-sm mt-4">Cached retries: {status.signer.cached_retries} · coalesced: {status.signer.retry_coalesced} · throttled: {status.signer.retry_throttled} · storage rejections: {status.signer.storage_rejections}</p>
    </section>
    <form
        class="card flex flex-col gap-4"
        onsubmit={(event) => {
            event.preventDefault();
            void saveRelays();
        }}
    >
        <div>
            <h2 class="text-xl font-bold">Relay configuration</h2>
            <p class="text-sm text-muted">
                One WebSocket URL per line. Saving replaces the enabled relay
                set.
            </p>
        </div>
        <label class="flex flex-col gap-2">
            <span>Enabled relays</span>
            <textarea rows="5" bind:value={relayLines} required></textarea>
        </label>
        <label class="flex flex-col gap-2 max-w-xs">
            <span>Minimum connected relays for readiness</span>
            <input
                type="number"
                min="1"
                bind:value={minimumConnectedRelays}
                required
            />
        </label>
        <button
            type="submit"
            class="button button-primary self-start"
            disabled={isSaving}
        >
            {isSaving ? "Saving…" : "Save and reconnect"}
        </button>
    </form>
{/if}
