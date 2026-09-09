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
    // Profile pictures come from arbitrary hosts; upgrade-insecure-requests
    // rewrites any http: URL, so only https: needs to be allowed.
    "img-src": ["self", "data:", "blob:", "https:"],
    "font-src": ["self", "data:"],
    // The UI talks to its own origin and to Nostr relays over wss:. A blanket
    // https: source would be an open exfiltration channel if XSS ever landed.
    "connect-src": ["self", "wss:"],
    "upgrade-insecure-requests": true,
};

/**
 * Permit exactly the configured API origin.
 *
 * `connect-src` no longer carries a blanket `https:`, so a split-origin
 * deployment must name its API explicitly or every management request is
 * blocked. Same-origin deployments are already covered by `'self'`; adding the
 * origin again is harmless. HTTP is accepted only on loopback, where there is no
 * TLS endpoint to upgrade to.
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
    if (url.protocol !== "http:" && url.protocol !== "https:") return directives;
    if (
        url.protocol === "http:" &&
        !["localhost", "127.0.0.1", "[::1]"].includes(url.hostname)
    ) {
        throw new Error("HTTP API origins are supported only on loopback");
    }
    const sources = directives["connect-src"];
    if (Array.isArray(sources) && !sources.includes(url.origin)) {
        sources.push(url.origin);
    }
    if (url.protocol === "http:") {
        delete directives["upgrade-insecure-requests"];
    }
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
