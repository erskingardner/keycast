<script lang="ts">
import type { NDKUser, NDKUserProfile } from "@nostr-dev-kit/ndk";

let {
    user,
    userProfile,
    npubMaxLength = 9,
}: {
    user: NDKUser;
    userProfile?: NDKUserProfile;
    npubMaxLength?: number;
} = $props();

let profile = $state<NDKUserProfile | null | undefined>(null);

$effect(() => {
    const currentProfile = userProfile || user.profile;
    if (currentProfile) {
        profile = currentProfile;
        return;
    }

    let cancelled = false;
    user.fetchProfile().then((profileResponse: NDKUserProfile | null) => {
        if (!cancelled) {
            profile = profileResponse;
        }
    });

    return () => {
        cancelled = true;
    };
});

let displayName = $derived(
    profile?.displayName || profile?.name || user.npub.slice(0, npubMaxLength)
);
</script>

{displayName}
