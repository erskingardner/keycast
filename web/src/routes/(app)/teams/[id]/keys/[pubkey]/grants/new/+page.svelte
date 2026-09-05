<script lang="ts">
    import { page } from "$app/stores";
    import GrantEditor from "$lib/components/GrantEditor.svelte";
</script>

<div class="standalone-form">
    <a class="description bordered" href={`/teams/${$page.params.id}`}
        >Back to workspace</a
    >
    <div class="mt-6">
        {#key $page.url.pathname}<GrantEditor
                id={$page.params.id ?? ""}
                pubkey={$page.params.pubkey ?? ""}
            />{/key}
    </div>
</div>
