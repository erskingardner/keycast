<script lang="ts">
    import TeamRoute from "$lib/components/TeamRoute.svelte";
    import { page } from "$app/stores";
    import KeyPanel from "$lib/components/KeyPanel.svelte";
</script>

<TeamRoute>
    {#snippet children(id, teamUrl)}
        <a class="description bordered" href={teamUrl}>Back to workspace</a>
        <h1 class="page-header mt-4 mb-6">Key access</h1>
        {#key $page.url.pathname}<KeyPanel
                {id}
                pubkey={$page.params.pubkey ?? ""}
            />{/key}
    {/snippet}
</TeamRoute>
