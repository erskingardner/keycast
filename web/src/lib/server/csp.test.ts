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
        for (const api of [undefined, "https://keycast.example"]) {
            const policy = createCspDirectives(api);
            expect(policy["upgrade-insecure-requests"]).toBe(true);
            expect(policy["connect-src"]).toEqual([
                "self",
                "https:",
                "wss:",
                "ws:",
            ]);
        }
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
