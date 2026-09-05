<script lang="ts">
    import { page } from "$app/stores";
    import PolicyEditor from "$lib/components/PolicyEditor.svelte";
</script>

<div class="standalone-form">
    <a class="description bordered" href={`/teams/${$page.params.id}`}
        >Back to workspace</a
    >
    <div class="mt-6">
        {#key $page.url.href}<PolicyEditor
                id={$page.params.id ?? ""}
                policyId={Number($page.url.searchParams.get("edit"))}
            />{/key}
    </div>
</div>
