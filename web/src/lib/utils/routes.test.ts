import { describe, expect, test } from "bun:test";
import { isProtectedRoute } from "./routes";

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
