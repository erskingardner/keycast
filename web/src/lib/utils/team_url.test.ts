import { expect, test } from "bun:test";
import type { TeamWithRelations } from "$lib/types";
import {
    resolveTeam,
    teamPath,
    teamSectionPath,
    workspaceSection,
} from "./team_url";

const teams: TeamWithRelations[] = [
    {
        team: {
            id: 1,
            name: "Personal",
            slug: "personal",
            created_at: 0,
            updated_at: 0,
        },
        stored_keys: [],
        policies: [],
        team_users: [],
    },
    {
        team: {
            id: 2,
            name: "Personal",
            slug: "personal-2",
            created_at: 0,
            updated_at: 0,
        },
        stored_keys: [],
        policies: [],
        team_users: [],
    },
];
test("team links use stored slugs and resolve numeric bookmarks", () => {
    expect(teamPath(teams[0].team)).toBe("/teams/personal");
    expect(resolveTeam(teams, "personal-2")?.team.id).toBe(2);
    expect(resolveTeam(teams, "2")?.team.slug).toBe("personal-2");
    expect(resolveTeam(teams.slice(1), "personal")).toBeUndefined();
    expect(resolveTeam(teams, "unavailable")).toBeUndefined();
});
test("renames preserve links and URL encoding handles international names", () => {
    const renamed = { ...teams[0].team, name: "Renamed" };
    expect(teamPath(renamed)).toBe("/teams/personal");
    expect(teamPath({ id: 3, slug: "café-東京" })).toBe(
        "/teams/caf%C3%A9-%E6%9D%B1%E4%BA%AC",
    );
    expect(teamPath({ id: 3, slug: null })).toBe("/teams/3");
});

test("workspace section links remain shareable and unknown sections default to keys", () => {
    expect(teamSectionPath(teams[1].team, "activity")).toBe(
        "/teams/personal-2?section=activity",
    );
    expect(
        workspaceSection(
            new URL(
                "https://example.com/teams/personal-2?section=activity",
            ).searchParams.get("section"),
        ),
    ).toBe("activity");
    expect(workspaceSection("settings")).toBe("settings");
    expect(workspaceSection(null)).toBe("keys");
    expect(workspaceSection("__proto__")).toBe("keys");
});
