<script lang="ts">
    import TeamRoute from "$lib/components/TeamRoute.svelte";
    import { page } from "$app/stores";
    import PolicyEditor from "$lib/components/PolicyEditor.svelte";
</script>

<TeamRoute>
    {#snippet children(id, teamUrl)}
        <div class="standalone-form">
            <a class="description bordered" href={teamUrl}>Back to workspace</a>
            <div class="mt-6">
                {#key $page.url.href}<PolicyEditor
                        {id}
                        policyId={Number($page.url.searchParams.get("edit"))}
                    />{/key}
            </div>
        </div>
    {/snippet}
</TeamRoute>
