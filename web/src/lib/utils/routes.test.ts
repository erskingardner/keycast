import { describe, expect, test } from "bun:test";
import { isProtectedRoute, signinDestination } from "./routes";

describe("route protection", () => {
    test("protects nested app routes, not just exact top-level paths", () => {
        expect(isProtectedRoute("/teams")).toBe(true);
        expect(isProtectedRoute("/teams/1")).toBe(true);
        expect(isProtectedRoute("/teams/1/keys/new")).toBe(true);
        expect(isProtectedRoute("/keys")).toBe(true);
        expect(isProtectedRoute("/keys/abc")).toBe(true);
    });

    test("does not treat similar public paths as protected", () => {
        expect(isProtectedRoute("/")).toBe(false);
        expect(isProtectedRoute("/team")).toBe(false);
        expect(isProtectedRoute("/teams-public")).toBe(false);
    });
});

describe("sign-in destination", () => {
    test("retains direct team and nested workspace links", () => {
        expect(signinDestination("/teams/12")).toBe("/teams/12");
        expect(signinDestination("/teams/12/keys/abc?tab=access")).toBe(
            "/teams/12/keys/abc?tab=access",
        );
    });
    test("rejects external, malformed and unrelated redirect destinations", () => {
        for (const value of [
            null,
            "https://evil.example/teams",
            "//evil.example/teams",
            "/\\evil.example/teams",
            "/\nevil.example/teams",
            "/teams/../../signin",
            "/teams-public",
            "/",
            "javascript:alert(1)",
        ]) {
            expect(signinDestination(value)).toBe("/teams");
        }
    });
});
