import { expect, test } from "bun:test";
import { relayHealth } from "./relay_health";
import type { RelayStatus } from "$lib/types";

const relay: RelayStatus = {
    id: 1,
    url: "wss://relay.example",
    enabled: true,
    sort_order: 0,
    last_connected_at: null,
    last_received_at: null,
    last_published_at: null,
    consecutive_failures: 3,
    last_error: "disconnected",
    reliability: null,
    diagnostics: {
        connection: "connected",
        subscription: "idle",
        observed_at: 1,
        connected_at: 1,
        attempts: 1,
        successes: 1,
        bytes_sent: 0,
        bytes_received: 0,
        latency_ms: null,
        subscription_error: null,
        transport_error: null,
        history: [],
    },
};
test("idle connected relays are healthy despite stale subscription checkpoints", () => {
    expect(relayHealth(relay)).toEqual({
        label: "Connected · idle",
        healthy: true,
    });
});
test("socket connectivity does not imply an accepted signing subscription", () => {
    expect(
        relayHealth({
            ...relay,
            diagnostics: { ...relay.diagnostics!, subscription: "rejected" },
        }).healthy,
    ).toBe(false);
    expect(
        relayHealth({
            ...relay,
            diagnostics: { ...relay.diagnostics!, subscription: "pending" },
        }).healthy,
    ).toBe(false);
    expect(
        relayHealth({
            ...relay,
            diagnostics: { ...relay.diagnostics!, connection: "disconnected" },
        }).healthy,
    ).toBe(false);
    expect(relayHealth({ ...relay, diagnostics: null }).label).toBe(
        "Awaiting live status",
    );
    expect(relayHealth({ ...relay, enabled: false }).label).toBe("Disabled");
});
