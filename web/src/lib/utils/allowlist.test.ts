import { describe, expect, test } from "bun:test";
import { checkPubkeyAllowed } from "./allowlist";

describe("pubkey allowlist helpers", () => {
    test("asks public API config whether one pubkey is allowed", async () => {
        const requestedUrls: string[] = [];
        const fetcher = async (input: RequestInfo | URL) => {
            requestedUrls.push(String(input));
            return new Response(JSON.stringify({ pubkey_allowed: true }), {
                status: 200,
            });
        };

        await expect(checkPubkeyAllowed("ABC 123", fetcher)).resolves.toBe(true);
        expect(requestedUrls).toEqual([
            "http://localhost:3100/api/config?pubkey=ABC+123",
        ]);
    });

    test("does not treat malformed public API config as allowed", async () => {
        const fetcher = async () =>
            new Response(JSON.stringify({ allowed_pubkeys: ["abc"] }), {
                status: 200,
            });

        await expect(checkPubkeyAllowed("abc", fetcher)).resolves.toBe(false);
    });

    test("fails closed when public API config cannot be checked", async () => {
        const fetcher = async () => new Response("nope", { status: 503 });

        await expect(checkPubkeyAllowed("abc", fetcher)).rejects.toThrow("HTTP 503");
    });
});
