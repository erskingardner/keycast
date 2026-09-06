<script lang="ts">
    import {
        getCachedProfile,
        loadProfile,
        type NostrProfile,
    } from "$lib/nostr";
    import { safeRemoteImageUrl } from "$lib/utils/image_url";
    let {
        pubkey,
        userProfile,
        extraClasses = "size-9",
    }: {
        pubkey: string;
        userProfile?: NostrProfile | null;
        extraClasses?: string;
    } = $props();
    let profile = $state<NostrProfile | null>(null);
    let failed = $state(false);
    const imageUrl = $derived(
        safeRemoteImageUrl(profile?.picture || profile?.image),
    );
    $effect(() => {
        failed = false;
        if (userProfile !== undefined) {
            profile = userProfile;
            return;
        }
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

{#if imageUrl && !failed}
    <img
        src={imageUrl}
        alt=""
        referrerpolicy="no-referrer"
        loading="lazy"
        decoding="async"
        onerror={() => (failed = true)}
        class="object-cover rounded-sm shrink-0 {extraClasses}"
    />
{:else}
    <span
        aria-hidden="true"
        class="inline-grid place-items-center rounded-sm shrink-0 bg-accent/10 text-accent font-mono text-xs {extraClasses}"
        >{pubkey.slice(0, 2).toUpperCase()}</span
    >
{/if}
