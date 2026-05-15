import { normalizeApiBaseUrl } from "./http_auth";

type PublicConfig = {
    pubkey_allowed?: unknown;
};

type ConfigFetcher = (
    input: RequestInfo | URL,
    init?: RequestInit,
) => Promise<Response>;

export async function checkPubkeyAllowed(
    pubkey: string,
    fetcher: ConfigFetcher = fetch,
): Promise<boolean> {
    const params = new URLSearchParams({ pubkey });
    const response = await fetcher(`${defaultApiBaseUrl()}/config?${params}`, {
        headers: { Accept: "application/json" },
    });

    if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }

    const config = (await response.json()) as PublicConfig;
    return config.pubkey_allowed === true;
}

function defaultApiBaseUrl(): string {
    const configuredDomain =
        import.meta.env.VITE_DOMAIN ||
        (import.meta.env.DEV ? "http://localhost:3000" : undefined);

    return normalizeApiBaseUrl(configuredDomain);
}
