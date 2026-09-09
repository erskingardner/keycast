import { generateSecretKey, getPublicKey } from "nostr-tools/pure";
import { v2 as nip44 } from "nostr-tools/nip44";
import { sha256Hex } from "./utils/http_auth";
import {
    MANAGEMENT_KIND,
    MANAGEMENT_READ_KIND,
    containsSecretMarker,
    managementDescription,
} from "./utils/management";
import { verifyManagementReplyKey } from "./utils/reply_identity";
import type { EventTemplate, NostrEvent } from "applesauce-core/helpers";
import { getContext, setContext } from "svelte";
import { signNostrEvent } from "./nostr";
import {
    buildNip98Tags,
    normalizeApiBaseUrl,
    type HttpAuthMethod,
} from "./utils/http_auth";

export class KeycastApi {
    private baseUrl: string;
    private pending = new Map<
        string,
        { secret: Uint8Array; created: number; replyKey: string }
    >();
    private defaultHeaders: HeadersInit;

    constructor() {
        const configuredDomain =
            import.meta.env?.VITE_DOMAIN ||
            (import.meta.env?.DEV ? "http://localhost:3100" : undefined);
        this.baseUrl = normalizeApiBaseUrl(configuredDomain);
        this.defaultHeaders = {
            "Content-Type": "application/json",
            Accept: "application/json",
        };
    }

    private async request<T>(
        endpoint: string,
        options: RequestInit = {},
    ): Promise<T> {
        const url = `${this.baseUrl}${endpoint}`;
        const headers = new Headers(this.defaultHeaders);
        new Headers(options.headers).forEach((value, key) => headers.set(key, value));

        const authorization = new Headers(headers).get("authorization") ?? "";
        const pending = this.pending.get(authorization);
        this.pending.delete(authorization);
        if (options.method && options.method !== "GET" && !pending) {
            throw new Error("A fresh external management approval is required");
        }
        let response: Response;
        try {
            response = await fetch(url, { ...options, headers, signal: options.signal ?? AbortSignal.timeout(20000) });
            if (pending && response.ok) {
                const envelope = await response.json() as { encrypted_response: string; public_key: string };
                // Decrypt against the identity pinned when the approval was signed,
                // never the one the response claims: that is what makes NIP-44
                // authenticate the sender rather than merely hide the reply.
                if (envelope.public_key !== pending.replyKey) {
                    throw new Error(
                        "The management reply came from an unexpected identity. Compare the reply identity on the Status page with your host before retrying.",
                    );
                }
                const plaintext = nip44.decrypt(envelope.encrypted_response, nip44.utils.getConversationKey(pending.secret, pending.replyKey));
                const reply = JSON.parse(plaintext) as { status: number; body: string };
                response = new Response(reply.status === 204 ? null : reply.body, { status: reply.status });
            }
        } finally {
            pending?.secret.fill(0);
        }

        if (!response.ok) {
            let detail = response.statusText;
            try {
                const body = (await response.json()) as { error?: unknown };
                if (typeof body.error === "string") detail = body.error;
            } catch {
                // Keep the protocol status when the response is not JSON.
            }
            throw new Error(`HTTP ${response.status}: ${detail}`);
        }

        if (response.status === 204) {
            return null as T;
        }

        return response.json() as Promise<T>;
    }

    async get<T>(
        endpoint: string,
        options: {
            headers?: HeadersInit;
            params?: Record<string, string>;
        } = {},
    ): Promise<T> {
        const url = options.params
            ? `${endpoint}?${new URLSearchParams(options.params)}`
            : endpoint;
        return this.request<T>(url, { headers: options.headers });
    }

    async post<T>(
        endpoint: string,
        data?: unknown,
        options: { headers?: HeadersInit } = {},
    ): Promise<T> {
        return this.request<T>(endpoint, {
            method: "POST",
            body: data ? JSON.stringify(data) : undefined,
            headers: options.headers,
        });
    }

    async put<T>(
        endpoint: string,
        data?: unknown,
        options: { headers?: HeadersInit } = {},
    ): Promise<T> {
        return this.request<T>(endpoint, {
            method: "PUT",
            body: data ? JSON.stringify(data) : undefined,
            headers: options.headers,
        });
    }

    async delete<T>(
        endpoint: string,
        options: { headers?: HeadersInit } = {},
    ): Promise<T> {
        return this.request<T>(endpoint, {
            method: "DELETE",
            headers: options.headers,
        });
    }

    async buildUnsignedAuthEvent(
        url: string,
        method: HttpAuthMethod,
        body?: string,
    ): Promise<EventTemplate> {
        const tags = await buildNip98Tags(this.baseUrl, url, method, body);
        return {
            content: "",
            kind: MANAGEMENT_READ_KIND,
            created_at: Math.floor(Date.now() / 1000),
            tags,
        };
    }

    async buildAuthHeader(
        url: string,
        method: HttpAuthMethod,
        pubkey: string,
        body?: string,
    ): Promise<string> {
        const write = method !== "GET";
        const unsignedAuthEvent = await this.buildUnsignedAuthEvent(url, method, body);
        let responseSecret: Uint8Array | undefined;
        const config = await this.get<{ instance_id: string; authority_revision: number; management_reply_public_key: string }>("/config", { params: { pubkey } });
        if (!config.instance_id || !Number.isSafeInteger(config.authority_revision)) throw new Error("Signer management configuration is unavailable");
        unsignedAuthEvent.tags.push(["instance", config.instance_id]);
        let replyKey = "";
        if (write) {
            replyKey = verifyManagementReplyKey(config.management_reply_public_key);
            responseSecret = generateSecretKey();
            const nonce = Array.from(crypto.getRandomValues(new Uint8Array(32)), b => b.toString(16).padStart(2,"0")).join("");
            unsignedAuthEvent.kind = MANAGEMENT_KIND;
            unsignedAuthEvent.content = managementDescription(method, url, body ?? "");
            // Refuse before asking the key store to sign. The approval is shown by,
            // stored in, and for NIP-46 relayed to an external signer.
            if (containsSecretMarker(unsignedAuthEvent.content)) {
                responseSecret.fill(0);
                throw new Error(
                    "This change cannot be approved because its description would contain private key material. Remove the key material from the name and try again.",
                );
            }
            unsignedAuthEvent.tags = unsignedAuthEvent.tags.filter(t => t[0] !== "payload");
            unsignedAuthEvent.tags.push(["payload", await sha256Hex(body ?? "")], ["revision", String(config.authority_revision)], ["nonce", nonce], ["response", getPublicKey(responseSecret)]);
        }
        try {
            unsignedAuthEvent.created_at = Math.floor(Date.now() / 1000);
            const signedAuthEvent = await signNostrEvent(unsignedAuthEvent, pubkey);
            const header = `Nostr ${base64Json(signedAuthEvent)}`;
            for (const [key,value] of this.pending) {
                if (Date.now() - value.created > 120000 || this.pending.size >= 16) {
                    value.secret.fill(0); this.pending.delete(key);
                }
            }
            if (responseSecret) {
                this.pending.set(header,{secret:responseSecret,created:Date.now(),replyKey});
                setTimeout(() => { const value = this.pending.get(header); if (value) { value.secret.fill(0); this.pending.delete(header); } }, 120000);
            }
            return header;
        } catch (error) {
            responseSecret?.fill(0);
            throw error;
        }
    }
}

function base64Json(event: NostrEvent): string {
    const json = JSON.stringify(event);
    if (typeof btoa === "function") {
        return btoa(Array.from(new TextEncoder().encode(json), byte => String.fromCharCode(byte)).join(""));
    }

    return Buffer.from(json, "utf-8").toString("base64");
}

const API_CONTEXT_KEY = Symbol("API");

export function initApi() {
    return setContext(API_CONTEXT_KEY, new KeycastApi());
}

export function getApi() {
    return getContext<ReturnType<typeof initApi>>(API_CONTEXT_KEY);
}
