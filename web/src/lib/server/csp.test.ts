import { describe, expect, test } from "bun:test";
import { createCspDirectives } from "./csp.js";

describe("configured API content security policy", () => {
    test("permits the exact custom loopback API origin without upgrading to nonexistent TLS", () => {
        for (const origin of [
            "http://localhost:3101",
            "http://127.0.0.1:4100",
            "http://[::1]:3101",
        ]) {
            const policy = createCspDirectives(`${origin}/api`);
            expect(policy["connect-src"]).toContain(origin);
            expect(policy["upgrade-insecure-requests"]).toBeUndefined();
            expect(policy["script-src"]).toEqual(["self"]);
            expect(policy["object-src"]).toEqual(["none"]);
        }
    });
    test("keeps production upgrade policy and never leaks local origins between configurations", () => {
        createCspDirectives("http://localhost:3101");
        const policy = createCspDirectives(undefined);
        expect(policy["upgrade-insecure-requests"]).toBe(true);
        expect(policy["connect-src"]).toEqual(["self", "wss:"]);
        expect(policy["img-src"]).not.toContain("http:");
    });
    test("permits a split-origin HTTPS API without restoring a blanket https: source", () => {
        // Without this the UI cannot reach /config or any management endpoint
        // when the API is served from a different origin.
        const policy = createCspDirectives("https://api.keycast.example/api");
        expect(policy["connect-src"]).toEqual([
            "self",
            "wss:",
            "https://api.keycast.example",
        ]);
        expect(policy["connect-src"]).not.toContain("https:");
        // HTTPS keeps the upgrade directive; only loopback HTTP drops it.
        expect(policy["upgrade-insecure-requests"]).toBe(true);
        // A same-origin API is already covered by 'self' and is not duplicated.
        const sameOrigin = createCspDirectives("https://keycast.example/api");
        const sources = sameOrigin["connect-src"];
        expect(Array.isArray(sources)).toBe(true);
        expect(
            (sources as string[]).filter(
                (source: string) => source === "https://keycast.example",
            ),
        ).toHaveLength(1);
        // Configurations must not leak into one another.
        expect(createCspDirectives("https://other.example")["connect-src"]).not.toContain(
            "https://api.keycast.example",
        );
    });
    test("rejects HTTP origins that only resemble loopback", () => {
        for (const api of [
            "http://localhost.attacker.example:3101",
            "http://127.0.0.1.attacker.example",
            "http://192.168.1.2:3101",
        ]) {
            expect(() => createCspDirectives(api)).toThrow("only on loopback");
        }
    });
});
