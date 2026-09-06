-- Diagnostic data only: no relay-supplied strings, Nostr payloads, or credentials.
CREATE TABLE relay_reliability_totals (
    relay_id INTEGER NOT NULL REFERENCES relays(id) ON DELETE CASCADE,
    category TEXT NOT NULL,
    count INTEGER NOT NULL CHECK(count >= 0),
    first_at INTEGER NOT NULL,
    last_at INTEGER NOT NULL,
    PRIMARY KEY(relay_id, category)
) STRICT;

CREATE TABLE relay_reliability_hours (
    relay_id INTEGER NOT NULL REFERENCES relays(id) ON DELETE CASCADE,
    hour INTEGER NOT NULL,
    category TEXT NOT NULL,
    count INTEGER NOT NULL CHECK(count >= 0),
    PRIMARY KEY(relay_id, hour, category)
) STRICT;
CREATE INDEX relay_reliability_hours_age ON relay_reliability_hours(hour);

-- Repeated events are grouped by minute; also capped at 1,000 rows per relay.
CREATE TABLE relay_reliability_events (
    relay_id INTEGER NOT NULL REFERENCES relays(id) ON DELETE CASCADE,
    minute INTEGER NOT NULL,
    category TEXT NOT NULL,
    count INTEGER NOT NULL CHECK(count > 0),
    first_at INTEGER NOT NULL,
    last_at INTEGER NOT NULL,
    PRIMARY KEY(relay_id, minute, category)
) STRICT;
CREATE INDEX relay_reliability_events_age ON relay_reliability_events(last_at);
