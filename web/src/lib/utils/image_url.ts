const REMOTE_IMAGE_PROTOCOLS = new Set(["http:", "https:"]);

export function safeRemoteImageUrl(rawUrl: unknown): string | null {
    if (typeof rawUrl !== "string") {
        return null;
    }

    const trimmedUrl = rawUrl.trim();
    if (!trimmedUrl) {
        return null;
    }

    try {
        const url = new URL(trimmedUrl);
        return REMOTE_IMAGE_PROTOCOLS.has(url.protocol) ? url.href : null;
    } catch {
        return null;
    }
}
