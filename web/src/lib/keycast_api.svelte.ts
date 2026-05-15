import type { EventTemplate, NostrEvent } from "applesauce-core/helpers";
import { getContext, setContext } from "svelte";
import { signNostrEvent } from "./nostr";
import {
    buildNip98Tags,
    normalizeApiBaseUrl,
    type HttpAuthMethod,
} from "./utils/http_auth";

const NIP_98_HTTP_AUTH_KIND = 27235;

export class KeycastApi {
    private baseUrl: string;
    private defaultHeaders: HeadersInit;

    constructor() {
        const configuredDomain =
            import.meta.env.VITE_DOMAIN ||
            (import.meta.env.DEV ? "http://localhost:3100" : undefined);
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
        const headers = { ...this.defaultHeaders, ...options.headers };

        const response = await fetch(url, { ...options, headers });

        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
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
            kind: NIP_98_HTTP_AUTH_KIND,
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
        const unsignedAuthEvent = await this.buildUnsignedAuthEvent(
            url,
            method,
            body,
        );
        const signedAuthEvent = await signNostrEvent(unsignedAuthEvent, pubkey);

        return `Nostr ${base64Json(signedAuthEvent)}`;
    }
}

function base64Json(event: NostrEvent): string {
    const json = JSON.stringify(event);
    if (typeof btoa === "function") {
        return btoa(json);
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
