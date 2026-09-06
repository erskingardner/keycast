<script lang="ts">
    import { npubEncode } from "applesauce-core/helpers";
    import Copy from "./Copy.svelte";
    import UserIdentity from "./UserIdentity.svelte";

    let { pubkey }: { pubkey: string } = $props();
    const npub = $derived(npubEncode(pubkey));
</script>

<div class="min-w-0 grow">
    <div class="mb-3"><UserIdentity {pubkey} showNpub={false} /></div>
    <p class="eyebrow mb-2">Public key</p>
    <dl
        class="grid grid-cols-[auto_minmax(0,1fr)] items-start gap-x-3 gap-y-2 text-xs"
    >
        <dt class="text-muted font-mono pt-1">npub</dt>
        <dd class="flex items-start gap-2 min-w-0">
            <span class="font-mono break-all select-all pt-1 min-w-0">{npub}</span>
            <Copy value={npub} label="Copy npub" />
        </dd>
        <dt class="text-muted font-mono pt-1">hex</dt>
        <dd class="flex items-start gap-2 min-w-0">
            <span class="font-mono break-all select-all pt-1 min-w-0">{pubkey}</span>
            <Copy value={pubkey} label="Copy hex" />
        </dd>
    </dl>
</div>
