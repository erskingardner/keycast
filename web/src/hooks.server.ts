import type { Handle } from "@sveltejs/kit";
import { redirect } from "@sveltejs/kit";
import { isProtectedRoute } from "$lib/utils/routes";

const csp = [
    "default-src 'self'",
    "base-uri 'self'",
    "object-src 'none'",
    "frame-ancestors 'none'",
    "form-action 'self'",
    "script-src 'self'",
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data:",
    "font-src 'self' data:",
    "connect-src 'self' https: wss:",
    "upgrade-insecure-requests",
].join("; ");

export const handle: Handle = async ({ event, resolve }) => {
    const sessionCookie = event.cookies.get("keycastUserPubkey");
    if (!sessionCookie && isProtectedRoute(event.url.pathname)) {
        throw redirect(303, "/");
    }

    const response = await resolve(event);

    response.headers.set("Content-Security-Policy", csp);
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
