import { normalizeApiBaseUrl } from "./http_auth";

type RawPubkeyAllowlist = string | readonly string[] | null | undefined;

type PublicConfig = {
    allowed_pubkeys?: unknown;
};

type ConfigFetcher = (
    input: RequestInfo | URL,
    init?: RequestInit,
) => Promise<Response>;

export function parsePubkeyAllowlist(rawAllowlist: RawPubkeyAllowlist): string[] {
    const values = Array.isArray(rawAllowlist)
        ? rawAllowlist
        : String(rawAllowlist ?? "").split(",");

    return values
        .map((value) => value.trim())
        .filter(Boolean);
}

export function isPubkeyAllowed(pubkey: string, rawAllowlist: RawPubkeyAllowlist): boolean {
    const allowedPubkeys = parsePubkeyAllowlist(rawAllowlist);

    if (allowedPubkeys.length === 0) return true;

    return allowedPubkeys.some(
        (allowedPubkey) => allowedPubkey.toLowerCase() === pubkey.toLowerCase(),
    );
}

export async function fetchPubkeyAllowlist(
    fetcher: ConfigFetcher = fetch,
): Promise<string[]> {
    const response = await fetcher(`${defaultApiBaseUrl()}/config`, {
        headers: { Accept: "application/json" },
    });

    if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }

    const config = (await response.json()) as PublicConfig;
    return parsePubkeyAllowlist(normalizeConfigAllowlist(config.allowed_pubkeys));
}

function defaultApiBaseUrl(): string {
    const configuredDomain =
        import.meta.env.VITE_DOMAIN ||
        (import.meta.env.DEV ? "http://localhost:3000" : undefined);

    return normalizeApiBaseUrl(configuredDomain);
}

function normalizeConfigAllowlist(value: unknown): RawPubkeyAllowlist {
    if (Array.isArray(value)) {
        return value.filter((entry): entry is string => typeof entry === "string");
    }

    if (typeof value === "string") {
        return value;
    }

    return undefined;
}
