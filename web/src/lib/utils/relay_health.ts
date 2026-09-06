import type { RelayStatus } from "$lib/types";

export function relayHealth(relay: RelayStatus): {
    label: string;
    healthy: boolean;
} {
    if (!relay.enabled) return { label: "Disabled", healthy: false };
    const live = relay.diagnostics;
    if (!live) return { label: "Awaiting live status", healthy: false };
    if (live.connection !== "connected")
        return { label: live.connection, healthy: false };
    if (live.subscription === "rejected")
        return { label: "Connected · subscription rejected", healthy: false };
    if (live.subscription === "pending")
        return { label: "Connected · subscription pending", healthy: false };
    return {
        label:
            live.subscription === "idle"
                ? "Connected · idle"
                : "Connected · signing ready",
        healthy: true,
    };
}
