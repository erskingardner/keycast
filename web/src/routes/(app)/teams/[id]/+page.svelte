<script lang="ts">
    import { page } from "$app/stores";
    import { goto } from "$app/navigation";
    import { getCurrentUser } from "$lib/current_user.svelte";
    import TeamWorkspace from "$lib/components/TeamWorkspace.svelte";
    const user = $derived(getCurrentUser()?.user);
</script>

<a class="description bordered" href="/teams">All teams</a>
<div class="mt-6">
    {#if user}{#key `${$page.params.id}:${user.pubkey}`}<TeamWorkspace
                id={$page.params.id ?? ""}
                onDeleted={() => {
                    void goto("/teams");
                }}
            />{/key}{/if}
</div>
