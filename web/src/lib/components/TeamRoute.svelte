<script lang="ts">
    import { page } from "$app/stores";
    import { goto } from "$app/navigation";
    import type { Snippet } from "svelte";
    import { getCurrentUser } from "$lib/current_user.svelte";
    import { KeycastApi } from "$lib/keycast_api.svelte";
    import type { TeamWithRelations } from "$lib/types";
    import { resolveTeam, teamPath } from "$lib/utils/team_url";
    import Loader from "./Loader.svelte";

    let { children }: { children: Snippet<[string, string]> } = $props();
    const user = $derived(getCurrentUser()?.user);
    let teams = $state<TeamWithRelations[]>([]);
    let loading = $state(true);
    let error = $state("");
    const selected = $derived(resolveTeam(teams, $page.params.id));
    const api = new KeycastApi();
    $effect(() => {
        const actor = user?.pubkey;
        if (!actor) return;
        let cancelled = false;
        loading = true;
        teams = [];
        error = "";
        void (async () => {
            try {
                const authorization = await api.buildAuthHeader(
                    "/teams",
                    "GET",
                    actor,
                );
                const result = await api.get<TeamWithRelations[]>("/teams", {
                    headers: { Authorization: authorization },
                });
                if (!cancelled) teams = result;
            } catch (e) {
                if (!cancelled)
                    error =
                        e instanceof Error ? e.message : "Could not load team";
            } finally {
                if (!cancelled) loading = false;
            }
        })();
        return () => {
            cancelled = true;
        };
    });
    $effect(() => {
        if (selected?.team.slug && $page.params.id !== selected.team.slug) {
            const suffix = $page.url.pathname.split("/").slice(3).join("/");
            void goto(
                `${teamPath(selected.team)}${suffix ? `/${suffix}` : ""}${$page.url.search}${$page.url.hash}`,
                { replaceState: true },
            );
        }
    });
</script>

{#if loading}<Loader />
{:else if error}<p class="input-error" role="alert">{error}</p>
{:else if selected}{@render children(
        String(selected.team.id),
        teamPath(selected.team),
    )}
{:else}<p role="alert" class="input-error">
        This team does not exist or your account does not have access.
    </p>
    <a href="/teams" class="button button-secondary mt-4">Go to your teams</a
    >{/if}
