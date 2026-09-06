<script lang="ts">
    import type { RelayReliability, RelayReliabilityCounts } from "$lib/types";
    import { formattedUnixDateTime } from "$lib/utils/dates";
    let { reliability }: { reliability: RelayReliability | null } = $props();
    const rows: [keyof RelayReliabilityCounts, string][] = [
        ["attempts", "Connection attempts"],
        ["retries", "Retries"],
        ["connections", "Successful connections"],
        ["remote_closes", "Remote close frames"],
        ["connection_losses", "Network / transport disconnects"],
        ["errors", "Errors / rejected requests"],
        ["cancelled_attempts", "Cancelled or timed-out attempts"],
    ];
</script>

<div class="mt-5 border-t border-line pt-4">
    <h3 class="eyebrow mb-3">Relay reliability</h3>
    {#if reliability}
        <div class="overflow-x-auto">
            <table class="w-full text-xs">
                <thead><tr class="text-muted border-b border-line">
                    <th class="text-left font-normal pb-2">Observed activity</th>
                    <th class="text-right font-normal pb-2">Last 7 days</th>
                    <th class="text-right font-normal pb-2">Lifetime</th>
                </tr></thead>
                <tbody>{#each rows as [key, label]}<tr>
                    <th class="text-left font-normal py-1.5">{label}</th>
                    <td class="text-right font-mono">{reliability.last_7_days[key].toLocaleString()}</td>
                    <td class="text-right font-mono">{reliability.lifetime[key].toLocaleString()}</td>
                </tr>{/each}</tbody>
            </table>
        </div>
        <p class="description mt-3">
            {#if reliability.tracking_since}Tracking since {formattedUnixDateTime(reliability.tracking_since)}. {/if}
            Counters survive restarts. The seven-day view uses hourly buckets from {formattedUnixDateTime(reliability.window_start)}.
            A remote close can be routine maintenance; network failures cannot always be attributed to the relay.
            Retries exclude the first connection attempt after startup or re-enabling a relay.
        </p>
        {#if reliability.lifetime.dropped_observations > 0}<p class="input-error mt-2">
            {reliability.lifetime.dropped_observations.toLocaleString()} observations were lost because the diagnostic buffer filled. Counts may be incomplete.
        </p>{/if}
        <details class="mt-4">
            <summary class="cursor-pointer text-xs text-muted">Seven-day event breakdown</summary>
            <dl class="mt-3 space-y-2 text-xs">{#each reliability.categories as category}
                <div class="flex gap-4 justify-between"><dt>{category.message}</dt><dd class="font-mono">{category.count.toLocaleString()}</dd></div>
            {/each}</dl>
        </details>
        <h4 class="eyebrow mt-5 mb-2">Recent events</h4>
        <p class="description mb-3">
            Latest 50 groups. Repeated events are grouped by minute. History is kept for seven days, up to 1,000 groups per relay; trimming history preserves counters.
            Refresh status for new entries. Counters normally flush every second; an abrupt crash can lose unflushed observations.
        </p>
        {#if reliability.history.length}<ol class="space-y-2 text-[11px] font-mono">
            {#each reliability.history as entry}<li class="flex flex-wrap gap-x-3 gap-y-1 border-b border-line/50 pb-2">
                <time class="text-muted" datetime={new Date(entry.last_at * 1000).toISOString()} title={`First: ${formattedUnixDateTime(entry.first_at)}`}>
                    {formattedUnixDateTime(entry.last_at)}
                </time>
                <span class="break-words" class:text-warning={entry.category.startsWith("error_") || entry.category === "connection_lost"}>{entry.message}</span>
                {#if entry.count > 1}<span class="text-muted">×{entry.count.toLocaleString()}</span>{/if}
            </li>{/each}
        </ol>{:else}<p class="description">No retained events in the last seven days.</p>{/if}
    {:else}
        <p class="description">Reliability tracking starts with the next observed connection event. Refresh status in a few seconds.</p>
    {/if}
</div>
