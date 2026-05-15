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
export const cspDirectives = {
    "default-src": ["self"],
    "base-uri": ["self"],
    "object-src": ["none"],
    "frame-ancestors": ["none"],
    "form-action": ["self"],
    "script-src": ["self"],
    "style-src": ["self", "unsafe-inline"],
    "img-src": ["self", "data:", "blob:", "https:", "http:"],
    "font-src": ["self", "data:"],
    "connect-src": [
        "self",
        "https:",
        "wss:",
        "ws:",
        "http://localhost:3100",
        "http://127.0.0.1:3100",
        "http://localhost:3000",
        "http://127.0.0.1:3000",
    ],
    "upgrade-insecure-requests": true,
};

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
