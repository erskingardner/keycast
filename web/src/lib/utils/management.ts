// Private Keycast management kind; every Keycast NIP-46 grant denies it.
export const MANAGEMENT_KIND = 27236;
export const MANAGEMENT_READ_KIND = 27237;
export function managementDescription(method: string, path: string, body: string): string {
    if (method === "POST" && path.split("?")[0].endsWith("/keys")) {
        const value = JSON.parse(body) as { name: string };
        if (typeof value.name !== "string") throw new Error("Key name required");
        return `Import private key named ${value.name}`;
    }
    return body;
}
