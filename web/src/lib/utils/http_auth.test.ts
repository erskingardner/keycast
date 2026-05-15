import { describe, expect, test } from "bun:test";
import { buildNip98Tags, normalizeApiBaseUrl, sha256Hex } from "./http_auth";

describe("NIP-98 HTTP auth helpers", () => {
    test("normalizes configured domains into API base URLs", () => {
        expect(normalizeApiBaseUrl(undefined, undefined)).toBe("http://localhost:3100/api");
        expect(normalizeApiBaseUrl(undefined, "https://keycast.example.com")).toBe(
            "https://keycast.example.com/api",
        );
        expect(normalizeApiBaseUrl("keycast.example.com")).toBe(
            "https://keycast.example.com/api",
        );
        expect(normalizeApiBaseUrl("https://keycast.example.com/")).toBe(
            "https://keycast.example.com/api",
        );
    });

    test("hashes bodies with SHA-256 hex", async () => {
        expect(await sha256Hex("hello")).toBe(
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
        );
    });

    test("builds URL, method, and payload tags", async () => {
        await expect(
            buildNip98Tags("https://keycast.example.com/api", "teams", "POST", "{}"),
        ).resolves.toEqual([
            ["u", "https://keycast.example.com/api/teams"],
            ["method", "POST"],
            [
                "payload",
                "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
            ],
        ]);
    });
});
