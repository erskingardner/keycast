<script lang="ts">
import type { AuthorizationWithRelations } from "$lib/types";
import { formattedDateTime } from "$lib/utils/dates";
import { Check, Copy } from "phosphor-svelte";
import { toast } from "svelte-hot-french-toast";

let { authorization }: { authorization: AuthorizationWithRelations } = $props();

let copyConnectionSuccess = $state(false);

function copyConnectionString(authorization: AuthorizationWithRelations) {
    navigator.clipboard.writeText(authorization.bunker_connection_string);
    toast.success("Connection string copied to clipboard");
    copyConnectionSuccess = true;
    setTimeout(() => {
        copyConnectionSuccess = false;
    }, 2000);
}
</script>

<div class="card">
    <h3 class="font-mono text-sm">{authorization.authorization.secret}</h3>
    <button onclick={() => copyConnectionString(authorization)} class="flex flex-row gap-2 items-center justify-center button button-primary button-icon {copyConnectionSuccess ? 'bg-green-600! text-white! ring-green-600!' : ''} transition-all duration-200">
        {#if copyConnectionSuccess}
            <Check size="20" />
            Copied!
        {:else}
            <Copy size="20" />
            Copy connection string
        {/if}
    </button>
    <div class="grid grid-cols-[auto_1fr] gap-y-1 gap-x-2 text-xs text-gray-400">
        <span class="whitespace-nowrap">Redemptions:</span>
        <span>{authorization.users.length} / {authorization.authorization.max_uses || "∞"}</span>
        <span class="whitespace-nowrap">Expiration:</span>
        <span>{formattedDateTime(new Date(authorization.authorization.expires_at)) || "None"}</span>
        <span class="whitespace-nowrap">Relays:</span>
        <span>{authorization.authorization.relays.join(", ")}</span>
        <span class="whitespace-nowrap">Policy:</span>
        <span>{authorization.policy.name}</span>
    </div>
</div>
