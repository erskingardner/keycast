import { afterEach, beforeEach, describe, expect, mock, test } from "bun:test";
import { finalizeEvent, generateSecretKey, getPublicKey, verifyEvent } from "nostr-tools/pure";
import { v2 as nip44 } from "nostr-tools/nip44";
import { sha256Hex } from "./utils/http_auth";
import { MANAGEMENT_KIND } from "./utils/management";
const identity = generateSecretKey();
const pubkey = getPublicKey(identity);
import { setActiveSigner, clearActiveSigner } from "./nostr";
import { KeycastApi } from "./keycast_api.svelte";
beforeEach(() => setActiveSigner({kind:"extension",pubkey,signer:{getPublicKey:async()=>pubkey,signEvent:async(event)=>finalizeEvent(event,identity)}}));
const originalFetch = globalThis.fetch;
afterEach(() => { globalThis.fetch = originalFetch; clearActiveSigner(); });
function eventFrom(header: string) { return JSON.parse(Buffer.from(header.slice(6), "base64").toString("utf8")); }
function configuration() { return Response.json({ instance_id: "test-instance", authority_revision: 4 }); }
function encryptedReply(header: string, status: number, body: string) {
    const approval = eventFrom(header);
    const recipient = approval.tags.find((tag: string[]) => tag[0] === "response")[1];
    const sender = generateSecretKey();
    return Response.json({ public_key: getPublicKey(sender), encrypted_response: nip44.encrypt(JSON.stringify({status, body}), nip44.utils.getConversationKey(sender, recipient)) });
}
describe("management HTTP encryption and external approvals", () => {
    test("binds Unicode command bytes and consumes its reply key exactly once", async () => {
        const api = new KeycastApi(); let header = ""; let calls = 0;
        const body = { name: "Família 🗝️" };
        globalThis.fetch = mock(async (url: string | URL | Request, options?: RequestInit) => {
            if (String(url).includes("/config?")) return configuration();
            calls++;
            expect(options?.body).toBe(JSON.stringify(body));
            expect(new Headers(options?.headers).get("authorization")).toBe(header);
            return encryptedReply(header, 201, JSON.stringify({ id: 9 }));
        }) as unknown as typeof fetch;
        header = await api.buildAuthHeader("/teams", "POST", pubkey, JSON.stringify(body));
        const event = eventFrom(header);
        expect(verifyEvent(event)).toBe(true);
        expect(event.kind).toBe(MANAGEMENT_KIND);
        expect(event.content).toBe(JSON.stringify(body));
        expect(event.tags).toContainEqual(["payload", await sha256Hex(JSON.stringify(body))]);
        expect(event.tags).toContainEqual(["instance", "test-instance"]);
        expect(event.tags).toContainEqual(["revision", "4"]);
        expect(await api.post<{id:number}>("/teams", body, { headers: new Headers({ Authorization: header }) })).toEqual({id: 9});
        await expect(api.post("/teams", body, {headers: {Authorization: header}})).rejects.toThrow("fresh external management approval");
        expect(calls).toBe(1);
    });
    test("does not place imported secrets in the signed public event", async () => {
        globalThis.fetch = mock(async () => configuration()) as unknown as typeof fetch;
        const api = new KeycastApi();
        const body = JSON.stringify({name: "Personal", secret_key: "sensitive-test-fixture"});
        const header = await api.buildAuthHeader("/teams/1/keys", "POST", pubkey, body);
        expect(JSON.stringify(eventFrom(header))).not.toContain("sensitive-test-fixture");
        expect(eventFrom(header).content).toBe("Import private key named Personal");
        expect(eventFrom(header).tags).toContainEqual(["payload", await sha256Hex(body)]);
    });
    test("rejects a plaintext success and cannot reuse its decryption key after failure", async () => {
        const api = new KeycastApi();
        globalThis.fetch = mock(async (url: string | URL | Request) => String(url).includes("/config?") ? configuration() : Response.json({id: "forged"})) as unknown as typeof fetch;
        const header = await api.buildAuthHeader("/teams", "POST", pubkey, '{"name":"test"}');
        await expect(api.post("/teams", {name:"test"}, {headers: {Authorization: header}})).rejects.toThrow();
        await expect(api.post("/teams", {name:"test"}, {headers: {Authorization: header}})).rejects.toThrow("fresh external management approval");
    });
    test("uses encrypted inner status for errors and successful DELETE", async () => {
        const api = new KeycastApi(); let header = ""; let status = 403;
        globalThis.fetch = mock(async (url: string | URL | Request) => String(url).includes("/config?") ? configuration() : encryptedReply(header,status,status === 204 ? "" : '{"error":"Forbidden"}')) as unknown as typeof fetch;
        header = await api.buildAuthHeader("/teams/1", "DELETE", pubkey);
        await expect(api.delete("/teams/1", {headers: {Authorization:header}})).rejects.toThrow("HTTP 403: Forbidden");
        status = 204;
        header = await api.buildAuthHeader("/teams/1", "DELETE", pubkey);
        expect(await api.delete("/teams/1", {headers: {Authorization:header}})).toBeNull();
    });
    test("reads use NIP-98 without management tags or a pending key", async () => {
        globalThis.fetch = mock(async () => Response.json({ ok: true })) as unknown as typeof fetch;
        const api = new KeycastApi();
        const header = await api.buildAuthHeader("/teams", "GET", pubkey);
        const event = eventFrom(header);
        expect(event.kind).toBe(27235);
        expect(event.tags.some((t:string[])=>t[0]==="response")).toBe(false);
        expect(await api.get<{ok:boolean}>("/teams", {headers:{Authorization:header}})).toEqual({ok:true});
    });
    test("refuses unsafe authority revisions before requesting a signature", async () => {
        globalThis.fetch = mock(async () => Response.json({instance_id:"test",authority_revision: 9007199254740992})) as unknown as typeof fetch;
        await expect(new KeycastApi().buildAuthHeader("/teams","POST",pubkey,"{}")).rejects.toThrow("configuration is unavailable");
    });
});
