<script lang="ts">
    import Loader from "$lib/components/Loader.svelte";
    import { getCurrentUser } from "$lib/current_user.svelte";
    import { KeycastApi } from "$lib/keycast_api.svelte";
    import type { RelayStatus, StatusResponse } from "$lib/types";
    import { formattedUnixDateTime } from "$lib/utils/dates";
    import {
        ArrowClockwise,
        CheckCircle,
        WarningCircle,
    } from "phosphor-svelte";
    import { toast } from "svelte-hot-french-toast";

    const api = new KeycastApi();
    const user = $derived(getCurrentUser()?.user);
    let status: StatusResponse | null = $state(null);
    let isLoading = $state(true);
    let isSaving = $state(false);
    let loadError = $state<string | null>(null);
    let minimumConnectedRelays = $state(1);
    let relayLines = $state("");

    $effect(() => {
        if (user?.pubkey) void refresh();
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
            relayLines = status.relays
                .filter((relay) => relay.enabled)
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

    async function saveRelays() {
        if (!user?.pubkey) return;
        const relays = relayLines
            .split("\n")
            .map((url) => url.trim())
            .filter(Boolean)
            .map((url) => ({ url, enabled: true }));
        const body = {
            minimum_connected_relays: minimumConnectedRelays,
            relays,
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

    function relayHealthy(relay: RelayStatus): boolean {
        return (
            relay.enabled &&
            relay.last_connected_at !== null &&
            relay.last_error === null &&
            relay.consecutive_failures === 0
        );
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
        onclick={refresh}
        disabled={isLoading}
    >
        <ArrowClockwise size="20" /> Refresh
    </button>
</div>

{#if loadError}<p class="input-error">
        {loadError}. Instance status requires an operator.
    </p>{/if}

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
                        class="text-amber-800"
                        size="22"
                    />{/if}
                <h2 class="font-bold">Signer</h2>
            </div>
            <p>{status.signer.ready ? "Ready" : "Not ready"}</p>
            {#if status.signer.recovery_pending}<p class="text-amber-800">
                    Signing paused for restore review.
                </p>{/if}
            {#if status.signer.quarantined_grants > 0}<p class="text-amber-800">
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
                    class="text-amber-800"
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
        <div class="flex flex-col gap-3">
            {#each status.relays as relay}
                <div
                    class="flex flex-col md:flex-row md:items-center md:justify-between gap-1 border-b border-line pb-3 last:border-0"
                >
                    <div class="flex items-center gap-2">
                        {#if relayHealthy(relay)}<CheckCircle
                                class="text-accent"
                                size="18"
                            />{:else}<WarningCircle
                                class="text-amber-800"
                                size="18"
                            />{/if}
                        <code>{relay.url}</code>
                    </div>
                    <div class="text-sm text-muted md:text-right">
                        <div>
                            {relay.enabled
                                ? `failures: ${relay.consecutive_failures}`
                                : "disabled"} · connected {formattedUnixDateTime(
                                relay.last_connected_at,
                            )}
                        </div>
                        <div>
                            received {formattedUnixDateTime(
                                relay.last_received_at,
                            )} · published {formattedUnixDateTime(
                                relay.last_published_at,
                            )}
                        </div>
                        {#if relay.last_error}<div class="text-amber-800">
                                {relay.last_error}
                            </div>{/if}
                    </div>
                </div>
            {/each}
        </div>
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
