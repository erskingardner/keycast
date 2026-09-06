const quotedSources = new Set([
    "self",
    "none",
    "unsafe-inline",
    "unsafe-eval",
    "wasm-unsafe-eval",
    "strict-dynamic",
    "report-sample",
]);

/** @typedef {Record<string, string[] | true>} CspDirectiveMap */

/** @type {CspDirectiveMap} */
const baseDirectives = {
    "default-src": ["self"],
    "base-uri": ["self"],
    "object-src": ["none"],
    "frame-ancestors": ["none"],
    "form-action": ["self"],
    "script-src": ["self"],
    "style-src": ["self", "unsafe-inline"],
    "img-src": ["self", "data:", "blob:", "https:", "http:"],
    "font-src": ["self", "data:"],
    "connect-src": ["self", "https:", "wss:", "ws:"],
    "upgrade-insecure-requests": true,
};

/**
 * Permit only the configured local API origin for split-port development.
 * HTTP loopback has no TLS endpoint, so upgrading its requests would break it.
 * Production and remote HTTPS configurations retain the default policy.
 * @param {string | undefined} apiUrl
 * @returns {CspDirectiveMap}
 */
export function createCspDirectives(apiUrl) {
    const directives = Object.fromEntries(
        Object.entries(baseDirectives).map(([name, value]) => [
            name,
            Array.isArray(value) ? [...value] : value,
        ]),
    );
    if (!apiUrl) return directives;
    const url = new URL(apiUrl);
    if (url.protocol !== "http:") return directives;
    if (!["localhost", "127.0.0.1", "[::1]"].includes(url.hostname)) {
        throw new Error("HTTP API origins are supported only on loopback");
    }
    const sources = directives["connect-src"];
    if (Array.isArray(sources)) sources.push(url.origin);
    delete directives["upgrade-insecure-requests"];
    return directives;
}

export const cspDirectives = createCspDirectives(undefined);

/** @param {string} source */
const formatSource = (source) => {
    if (quotedSources.has(source)) {
        return `'${source}'`;
    }

    return source;
};

export const cspHeaderValue = Object.entries(cspDirectives)
    .map(([directive, value]) => {
        if (value === true) {
            return directive;
        }

        return `${directive} ${value.map(formatSource).join(" ")}`;
    })
    .join("; ");
