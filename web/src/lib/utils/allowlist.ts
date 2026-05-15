export function parsePubkeyAllowlist(rawAllowlist: string | null | undefined): string[] {
    return String(rawAllowlist ?? "")
        .split(",")
        .map((value) => value.trim())
        .filter(Boolean);
}

export function isPubkeyAllowed(pubkey: string, rawAllowlist: string | null | undefined): boolean {
    const allowedPubkeys = parsePubkeyAllowlist(rawAllowlist);

    if (allowedPubkeys.length === 0) return true;

    return allowedPubkeys.some(
        (allowedPubkey) => allowedPubkey.toLowerCase() === pubkey.toLowerCase(),
    );
}
