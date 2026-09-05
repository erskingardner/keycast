<script lang="ts">
    import { page } from "$app/stores";
    import KeyPanel from "$lib/components/KeyPanel.svelte";
</script>

<a class="description bordered" href={`/teams/${$page.params.id}`}
    >Back to workspace</a
>
<h1 class="page-header mt-4 mb-6">Key access</h1>
{#key $page.url.pathname}<KeyPanel
        id={$page.params.id ?? ""}
        pubkey={$page.params.pubkey ?? ""}
    />{/key}
