import type { Handle } from "@sveltejs/kit";
import { cspHeaderValue } from "$lib/server/csp.js";
import { isProtectedRoute } from "$lib/utils/routes";

function applySecurityHeaders(response: Response, event: Parameters<Handle>[0]["event"]) {
    if (!response.headers.has("Content-Security-Policy")) {
        response.headers.set("Content-Security-Policy", cspHeaderValue);
    }
    response.headers.set("Cache-Control", "no-store");
    response.headers.set("X-Content-Type-Options", "nosniff");
    response.headers.set("X-Frame-Options", "DENY");
    response.headers.set("Referrer-Policy", "no-referrer");
    response.headers.set("Permissions-Policy", "camera=(), microphone=(), geolocation=()");

    const forwardedProto = event.request.headers.get("x-forwarded-proto");
    if (event.url.protocol === "https:" || forwardedProto === "https") {
        response.headers.set(
            "Strict-Transport-Security",
            "max-age=31536000; includeSubDomains",
        );
    }
}

export const handle: Handle = async ({ event, resolve }) => {
    const sessionCookie = event.cookies.get("keycastUserPubkey");
    if (!sessionCookie && isProtectedRoute(event.url.pathname)) {
        const response = new Response(null, {
            status: 303,
            headers: { Location: `/?${new URLSearchParams({ returnTo: event.url.pathname + event.url.search })}` },
        });
        applySecurityHeaders(response, event);
        return response;
    }

    const response = await resolve(event);

    applySecurityHeaders(response, event);

    return response;
};
