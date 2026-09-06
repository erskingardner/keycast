import type { Team, TeamWithRelations } from "$lib/types";

export function teamPath(team: Pick<Team, "id" | "slug">): string {
    return `/teams/${encodeURIComponent(team.slug || String(team.id))}`;
}

export function resolveTeam(
    teams: TeamWithRelations[],
    reference?: string,
): TeamWithRelations | undefined {
    if (!reference) return undefined;
    return teams.find(
        ({ team }) => team.slug === reference || String(team.id) === reference,
    );
}

export const workspaceSections = [
    "keys",
    "policies",
    "members",
    "activity",
    "settings",
] as const;
export type WorkspaceSection = (typeof workspaceSections)[number];

export function workspaceSection(value: string | null): WorkspaceSection {
    return workspaceSections.find((section) => section === value) ?? "keys";
}

export function teamSectionPath(
    team: Pick<Team, "id" | "slug">,
    section: WorkspaceSection,
): string {
    return `${teamPath(team)}?section=${section}`;
}
