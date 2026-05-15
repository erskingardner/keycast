export type HttpAuthMethod = "GET" | "POST" | "PUT" | "DELETE";

export function normalizeApiBaseUrl(rawDomain: string | null | undefined): string {
    const raw = String(rawDomain || "http://localhost:3000").trim();
    const domain = /^https?:\/\//i.test(raw) ? raw : `https://${raw}`;

    return `${domain.replace(/\/+$/, "")}/api`;
}

export async function sha256Hex(body: string): Promise<string> {
    const buffer = await crypto.subtle.digest(
        "SHA-256",
        new TextEncoder().encode(body),
    );

    return Array.from(new Uint8Array(buffer))
        .map((byte) => byte.toString(16).padStart(2, "0"))
        .join("");
}

export async function buildNip98Tags(
    baseUrl: string,
    endpoint: string,
    method: HttpAuthMethod,
    body?: string,
): Promise<string[][]> {
    const path = endpoint.startsWith("/") ? endpoint : `/${endpoint}`;
    const tags = [
        ["u", `${baseUrl}${path}`],
        ["method", method],
    ];

    if (body) {
        tags.push(["payload", await sha256Hex(body)]);
    }

    return tags;
}
