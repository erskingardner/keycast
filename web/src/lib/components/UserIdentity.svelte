<script lang="ts">
    import Avatar from "./Avatar.svelte";
    import {
        getCachedProfile,
        loadProfile,
        npubForPubkey,
        type NostrProfile,
    } from "$lib/nostr";
    let {
        pubkey,
        isYou = false,
        showNpub = true,
    }: { pubkey: string; isYou?: boolean; showNpub?: boolean } = $props();
    let profile = $state<NostrProfile | null>(null);
    const npub = $derived(npubForPubkey(pubkey) ?? pubkey);
    const displayName = $derived(
        profile?.display_name || profile?.name || "Unnamed user",
    );
    $effect(() => {
        profile = getCachedProfile(pubkey);
        let cancelled = false;
        void loadProfile(pubkey).then((value) => {
            if (!cancelled) profile = value;
        });
        return () => {
            cancelled = true;
        };
    });
</script>

<span class="flex items-center gap-3 min-w-0">
    <Avatar {pubkey} userProfile={profile} />
    <span class="min-w-0">
        <span class="block text-sm font-medium truncate" title={displayName}
            >{displayName}{#if isYou}<span class="text-muted text-xs">
                    · you</span
                >{/if}</span
        >
        {#if showNpub}<span
                class="block font-mono text-[11px] text-muted truncate"
                title={npub}>{npub.slice(0, 16)}…{npub.slice(-8)}</span
            >{/if}
    </span>
</span>
