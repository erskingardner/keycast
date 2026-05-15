<script lang="ts">
import { loadProfile, type NostrProfile } from "$lib/nostr";
import { safeRemoteImageUrl } from "$lib/utils/image_url";

let {
    pubkey,
    userProfile,
    extraClasses,
}: { pubkey: string; userProfile?: NostrProfile | null; extraClasses: string } = $props();

let profile = $state<NostrProfile | null | undefined>(null);
let imageUrl = $derived(safeRemoteImageUrl(profile?.picture || profile?.image));
let fallbackImageUrl = $derived(`https://robohash.org/${encodeURIComponent(pubkey)}`);

$effect(() => {
    const currentProfile = userProfile;
    if (currentProfile) {
        profile = currentProfile;
        return;
    }

    let cancelled = false;
    loadProfile(pubkey).then((fetchedProfile) => {
        if (!cancelled) {
            profile = fetchedProfile;
        }
    });

    return () => {
        cancelled = true;
    };
});
</script>

{#if imageUrl}
    <img src={imageUrl} alt="Avatar" referrerpolicy="no-referrer" class="object-cover rounded-full {extraClasses} ring-1 ring-gray-300 dark:ring-gray-500" />
{:else}
    <img src={fallbackImageUrl} alt="No-Avatar" referrerpolicy="no-referrer" class="object-cover rounded-full {extraClasses} ring-1 ring-gray-300 dark:ring-gray-500" />
{/if}
