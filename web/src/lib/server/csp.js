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
    "img-src": ["self", "data:"],
    "font-src": ["self", "data:"],
    "connect-src": ["self", "https:", "wss:"],
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
