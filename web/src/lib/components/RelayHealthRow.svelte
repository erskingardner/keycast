<script lang="ts">
    import RelayReliabilityPanel from "$lib/components/RelayReliabilityPanel.svelte";
    import type { RelayStatus } from "$lib/types";
    import { formattedUnixDateTime } from "$lib/utils/dates";
    import { relayHealth } from "$lib/utils/relay_health";
    import { CaretDown, CheckCircle, WarningCircle } from "phosphor-svelte";
    let { relay }: { relay: RelayStatus } = $props();
    const health = $derived(relayHealth(relay));
    const live = $derived(relay.diagnostics);
    const timestamp = (value: number | null | undefined) =>
        value ? formattedUnixDateTime(value) : "Not yet recorded";
</script>

<details class="relay-row border-b border-line last:border-0">
    <summary
        class="flex cursor-pointer list-none flex-wrap items-center gap-3 py-4"
    >
        {#if health.healthy}<CheckCircle
                class="text-accent shrink-0"
                size={18}
            />{:else}<WarningCircle
                class="text-muted shrink-0"
                size={18}
            />{/if}
        <code class="min-w-0 grow break-all text-sm">{relay.url}</code>
        <span
            class="text-xs"
            class:text-accent={health.healthy}
            class:text-warning={!health.healthy && relay.enabled}
            class:text-muted={!relay.enabled}>{health.label}</span
        >
        {#if relay.reliability}
            <span class="text-muted text-[11px] basis-full pl-7 sm:basis-auto sm:pl-0">
                7d: {relay.reliability.last_7_days.remote_closes} remote closes · {relay.reliability.last_7_days.retries} retries · {relay.reliability.last_7_days.errors} errors
            </span>
        {/if}
        <CaretDown size={14} class="relay-chevron shrink-0 text-muted" />
    </summary>
    <div class="pb-5 pl-7">
        {#if live}
            <dl
                class="grid sm:grid-cols-2 lg:grid-cols-3 gap-x-6 gap-y-4 text-xs"
            >
                <div>
                    <dt class="eyebrow mb-1">Last connection</dt>
                    <dd>{timestamp(live.connected_at)}</dd>
                </div>
                <div>
                    <dt class="eyebrow mb-1">
                        This run: attempts / successes
                    </dt>
                    <dd class="font-mono">
                        {live.attempts} / {live.successes}
                    </dd>
                </div>
                <div>
                    <dt class="eyebrow mb-1">Latency</dt>
                    <dd>
                        {live.latency_ms === null
                            ? "Not measured yet"
                            : `${live.latency_ms} ms`}
                    </dd>
                </div>
                <div>
                    <dt class="eyebrow mb-1">Network traffic</dt>
                    <dd class="font-mono">
                        {live.bytes_received.toLocaleString()} B received · {live.bytes_sent.toLocaleString()}
                        B sent
                    </dd>
                </div>
                <div>
                    <dt class="eyebrow mb-1">Last signing event received</dt>
                    <dd>{timestamp(relay.last_received_at)}</dd>
                </div>
                <div>
                    <dt class="eyebrow mb-1">Last signing reply published</dt>
                    <dd>{timestamp(relay.last_published_at)}</dd>
                </div>
            </dl>
            <p class="description mt-4">
                {live.subscription === "idle"
                    ? "No active grants; no signing subscription is needed yet."
                    : live.subscription === "accepted"
                      ? "The relay has accepted the signing subscription."
                      : live.subscription === "rejected"
                        ? "The relay rejected the signing subscription. Keycast will retry with backoff."
                        : "Waiting for the relay to accept the signing subscription."}
            </p>
            {#if live.subscription_error}<p class="input-error mt-2">
                    {live.subscription_error}
                </p>{/if}
            {#if live.connection === "disconnected"}<p class="input-error mt-2">
                    {live.transport_error ?? "Connection lost or failed."} Automatic reconnection is enabled.
                </p>{/if}

        {:else}
            <p class="description">
                {relay.enabled
                    ? "Waiting for the signer's first connection check. Refresh status in a few seconds."
                    : "This relay is disabled."}
            </p>
            {#if relay.last_error}<p class="text-muted text-xs mt-2">
                    Previous recorded error: {relay.last_error}
                </p>{/if}
        {/if}
        <RelayReliabilityPanel reliability={relay.reliability} />
    </div>
</details>

<style>
    summary::-webkit-details-marker {
        display: none;
    }
    .relay-row[open] :global(.relay-chevron) {
        transform: rotate(180deg);
    }
</style>
