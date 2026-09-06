<script lang="ts">
    import TeamRoute from "$lib/components/TeamRoute.svelte";
    import { page } from "$app/stores";
    import MemberEditor from "$lib/components/MemberEditor.svelte";
</script>

<TeamRoute>
    {#snippet children(id, teamUrl)}
        <div class="standalone-form">
            <a class="description bordered" href={teamUrl}>Back to workspace</a>
            <div class="mt-6">
                {#key $page.url.pathname}<MemberEditor {id} />{/key}
            </div>
        </div>
    {/snippet}
</TeamRoute>
