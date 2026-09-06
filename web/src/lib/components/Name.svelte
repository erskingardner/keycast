<script lang="ts">
    import {
        loadProfile,
        getCachedProfile,
        npubForPubkey,
        type NostrProfile,
    } from "$lib/nostr";

    let {
        pubkey,
        userProfile,
        npubMaxLength = 9,
    }: {
        pubkey: string;
        userProfile?: NostrProfile | null;
        npubMaxLength?: number;
    } = $props();

    let profile = $state<NostrProfile | null | undefined>(null);

    $effect(() => {
        const currentProfile = userProfile;
        if (currentProfile !== undefined) {
            profile = currentProfile;
            return;
        }

        profile = getCachedProfile(pubkey);
        let cancelled = false;
        loadProfile(pubkey).then((profileResponse) => {
            if (!cancelled) {
                profile = profileResponse;
            }
        });

        return () => {
            cancelled = true;
        };
    });

    let displayName = $derived(
        profile?.display_name ||
            profile?.displayName ||
            profile?.name ||
            npubForPubkey(pubkey)?.slice(0, npubMaxLength) ||
            pubkey.slice(0, npubMaxLength),
    );
</script>

{displayName}
