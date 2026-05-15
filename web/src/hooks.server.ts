import type { Handle } from "@sveltejs/kit";
import { redirect } from "@sveltejs/kit";
import { isProtectedRoute } from "$lib/utils/routes";
import { cspHeaderValue } from "$lib/server/csp.js";

export const handle: Handle = async ({ event, resolve }) => {
    const sessionCookie = event.cookies.get("keycastUserPubkey");
    if (!sessionCookie && isProtectedRoute(event.url.pathname)) {
        throw redirect(303, "/");
    }

    const response = await resolve(event);

    if (!response.headers.has("Content-Security-Policy")) {
        response.headers.set("Content-Security-Policy", cspHeaderValue);
    }
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

    return response;
};
