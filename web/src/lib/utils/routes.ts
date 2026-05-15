const protectedRoutePrefixes = ["/teams", "/keys"];

export function isProtectedRoute(pathname: string): boolean {
    return protectedRoutePrefixes.some(
        (route) => pathname === route || pathname.startsWith(`${route}/`),
    );
}
