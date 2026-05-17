export type SignerKind = "extension" | "amber" | "remote";

export type StoredSignerSession =
    | {
          kind: "extension";
          pubkey: string;
      }
    | {
          kind: "amber";
          pubkey: string;
      }
    | {
          kind: "remote";
          pubkey: string;
          bunkerUri: string;
          clientSecretHex: string;
      };

const HEX_64 = /^[0-9a-f]{64}$/i;

export function isHexPubkey(value: unknown): value is string {
    return typeof value === "string" && HEX_64.test(value);
}

export function parseStoredSignerSession(
    raw: string | null | undefined,
): StoredSignerSession | null {
    if (!raw) return null;

    let value: unknown;
    try {
        value = JSON.parse(raw);
    } catch {
        return null;
    }

    if (!value || typeof value !== "object") return null;

    const session = value as Record<string, unknown>;
    if (!isHexPubkey(session.pubkey)) return null;

    if (session.kind === "extension" || session.kind === "amber") {
        return {
            kind: session.kind,
            pubkey: session.pubkey.toLowerCase(),
        };
    }

    if (
        session.kind === "remote" &&
        typeof session.bunkerUri === "string" &&
        session.bunkerUri.startsWith("bunker://") &&
        typeof session.clientSecretHex === "string" &&
        /^[0-9a-f]+$/i.test(session.clientSecretHex) &&
        session.clientSecretHex.length === 64
    ) {
        return {
            kind: "remote",
            pubkey: session.pubkey.toLowerCase(),
            bunkerUri: session.bunkerUri,
            clientSecretHex: session.clientSecretHex.toLowerCase(),
        };
    }

    return null;
}

export function serializeStoredSignerSession(
    session: StoredSignerSession,
): string {
    return JSON.stringify(session);
}
