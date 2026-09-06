const protectedRoutePrefixes = ["/teams", "/keys"];

export function isProtectedRoute(pathname: string): boolean {
    return protectedRoutePrefixes.some(
        (route) => pathname === route || pathname.startsWith(`${route}/`),
    );
}

/** Only return to a local workspace route after sign-in, never a supplied external URL. */
export function signinDestination(returnTo: string | null): string {
    if (
        !returnTo ||
        returnTo.length > 2048 ||
        !returnTo.startsWith("/") ||
        returnTo.startsWith("//") ||
        /[\\\u0000-\u0020]/.test(returnTo)
    )
        return "/teams";
    try {
        const base = "https://keycast.invalid";
        const url = new URL(returnTo, base);
        if (url.origin !== base || !isProtectedRoute(url.pathname))
            return "/teams";
        return url.pathname + url.search + url.hash;
    } catch {
        return "/teams";
    }
}
