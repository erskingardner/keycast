<script lang="ts">
    import { page } from "$app/stores";
    import MemberEditor from "$lib/components/MemberEditor.svelte";
</script>

<div class="standalone-form">
    <a class="description bordered" href={`/teams/${$page.params.id}`}
        >Back to workspace</a
    >
    <div class="mt-6">
        {#key $page.url.pathname}<MemberEditor
                id={$page.params.id ?? ""}
            />{/key}
    </div>
</div>
