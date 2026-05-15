export function parseAllowedKindsInput(value: string): number[] | null {
    const tokens = value
        .split(",")
        .map((token) => token.trim())
        .filter(Boolean);

    if (tokens.length === 0) return null;

    return tokens
        .map((token) => Number.parseInt(token, 10))
        .filter((kind) => Number.isInteger(kind) && kind >= 0 && kind <= 65535);
}

export function parseBlockedWordsInput(value: string): string[] | null {
    const words = value
        .split(",")
        .map((word) => word.trim())
        .filter(Boolean);

    return words.length === 0 ? null : words;
}
