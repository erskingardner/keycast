<script lang="ts">
import type { NDKUser, NDKUserProfile } from "@nostr-dev-kit/ndk";

let {
    user,
    userProfile,
    extraClasses,
}: { user: NDKUser; userProfile?: NDKUserProfile; extraClasses: string } = $props();

let profile = $state<NDKUserProfile | null | undefined>(null);

$effect(() => {
    const currentProfile = userProfile || user.profile;
    if (currentProfile) {
        profile = currentProfile;
        return;
    }

    let cancelled = false;
    user.fetchProfile().then((fetchedProfile) => {
        if (!cancelled) {
            profile = fetchedProfile;
        }
    });

    return () => {
        cancelled = true;
    };
});
</script>

{#if profile?.image}
    <img src={profile.image as string} alt="Avatar" class="object-cover rounded-full {extraClasses} ring-1 ring-gray-300 dark:ring-gray-500" />
{:else}
    <img src="https://robohash.org/{user.pubkey}" alt="No-Avatar" class="object-cover rounded-full {extraClasses} ring-1 ring-gray-300 dark:ring-gray-500" />
{/if}
