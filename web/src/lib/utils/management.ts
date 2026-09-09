// Private Keycast management kind; every Keycast NIP-46 grant denies it.
export const MANAGEMENT_KIND = 27236;
export const MANAGEMENT_READ_KIND = 27237;
/// Mirrors `contains_secret_marker` in core/src/v2/management.rs. An approval is
/// displayed by, stored in, and for NIP-46 transported to an external key store.
const SECRET_MARKERS = ["secret_key", "nsec1"];
export function containsSecretMarker(content: string): boolean {
    const lowered = content.toLowerCase();
    return SECRET_MARKERS.some((marker) => lowered.includes(marker));
}
export function managementDescription(method: string, path: string, body: string): string {
    if (method === "POST" && path.split("?")[0].endsWith("/keys")) {
        const value = JSON.parse(body) as { name: string };
        if (typeof value.name !== "string") throw new Error("Key name required");
        return `Import private key named ${value.name}`;
    }
    return body;
}
