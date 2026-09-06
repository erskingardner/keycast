import { describe, expect, mock, test } from "bun:test";
import { isProtectedRoute } from "./lib/utils/routes";

mock.module("$lib/utils/routes", () => ({
    isProtectedRoute,
}));

describe("security headers", () => {
    test("preserves a deep link through the sign-in redirect", async () => {
        const { handle } = await import("./hooks.server");
        const url = new URL("http://localhost/teams/12?tab=access");
        const event = { cookies: { get: () => undefined }, request: new Request(url), url } as unknown as Parameters<typeof handle>[0]["event"];
        const response = await handle({ event, resolve: async () => { throw new Error("Protected page should not render"); } });
        expect(response.status).toBe(303);
        expect(new URL(response.headers.get("Location")!, url).searchParams.get("returnTo")).toBe("/teams/12?tab=access");
        expect(response.headers.get("Cache-Control")).toBe("no-store");
    });
    test("preserves SvelteKit's generated CSP sources", async () => {
        const { handle } = await import("./hooks.server");
        const event = {
            cookies: {
                get: () => undefined,
            },
            request: new Request("http://localhost/"),
            url: new URL("http://localhost/"),
        } as unknown as Parameters<typeof handle>[0]["event"];
        const response = await handle({
            event,
            resolve: async () => {
                return new Response("", {
                    headers: {
                        "Content-Security-Policy":
                            "script-src 'self' 'nonce-sveltekit-generated'",
                    },
                });
            },
        });

        expect(response.headers.get("Content-Security-Policy")).toBe(
            "script-src 'self' 'nonce-sveltekit-generated'",
        );
        expect(response.headers.get("X-Content-Type-Options")).toBe("nosniff");
    });

    test("adds a fallback CSP when SvelteKit did not generate one", async () => {
        const { handle } = await import("./hooks.server");
        const event = {
            cookies: {
                get: () => undefined,
            },
            request: new Request("http://localhost/health"),
            url: new URL("http://localhost/health"),
        } as unknown as Parameters<typeof handle>[0]["event"];
        const response = await handle({
            event,
            resolve: async () =>
                new Response("{}", { headers: { "Content-Type": "application/json" } }),
        });

        expect(response.headers.get("Content-Security-Policy")).toContain(
            "script-src 'self'",
        );
        expect(response.headers.get("Content-Security-Policy")).toContain(
            "object-src 'none'",
        );
    });
});
