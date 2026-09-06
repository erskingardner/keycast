<script lang="ts">
    import { teamPath } from "$lib/utils/team_url";
    import type { TeamWithRelations } from "$lib/types";
    import { dateFromUnixSeconds, formattedDate } from "$lib/utils/dates";

    let { team }: { team: TeamWithRelations } = $props();
</script>

<a href={teamPath(team.team)} class="card hover-card">
    <h3 class="text-xl font-bold">{team.team.name}</h3>
    <div class="flex flex-col gap-1">
        <span class="text-sm text-muted font-mono">
            {team.stored_keys?.length} managed {`${team.stored_keys?.length === 1 ? "key" : "keys"}`}
        </span>
        <span class="text-sm text-muted font-mono">
            {team.team_users?.length}
            {`${team.team_users?.length === 1 ? "member" : "members"}`}
        </span>
    </div>
    <span class="text-xs text-muted"
        >Created: {formattedDate(
            dateFromUnixSeconds(team.team.created_at),
        )}</span
    >
</a>
