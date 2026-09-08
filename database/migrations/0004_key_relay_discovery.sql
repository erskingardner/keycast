-- Public metadata only. Network activation remains an instance-operator decision.
ALTER TABLE relays ADD COLUMN discovered INTEGER NOT NULL DEFAULT 0 CHECK(discovered IN (0,1));
CREATE TABLE relay_discovery_policy (
 singleton INTEGER PRIMARY KEY CHECK(singleton=1),
 auto_activate INTEGER NOT NULL DEFAULT 0 CHECK(auto_activate IN (0,1))
) STRICT;
INSERT INTO relay_discovery_policy(singleton) VALUES(1);
CREATE TABLE key_relay_lists (
 stored_key_id INTEGER PRIMARY KEY REFERENCES stored_keys(id) ON DELETE CASCADE,
 event_id TEXT,
 created_at INTEGER,
 fetched_at INTEGER,
 next_fetch_at INTEGER NOT NULL DEFAULT 0,
 status TEXT NOT NULL DEFAULT 'pending'
) STRICT;
INSERT INTO key_relay_lists(stored_key_id) SELECT id FROM stored_keys;
CREATE TRIGGER discover_imported_key AFTER INSERT ON stored_keys BEGIN
 INSERT INTO key_relay_lists(stored_key_id) VALUES(new.id);
END;
CREATE TABLE key_relays (
 stored_key_id INTEGER NOT NULL REFERENCES stored_keys(id) ON DELETE CASCADE,
 url TEXT NOT NULL,
 read INTEGER NOT NULL CHECK(read IN (0,1)),
 write INTEGER NOT NULL CHECK(write IN (0,1)),
 listed INTEGER NOT NULL DEFAULT 1 CHECK(listed IN (0,1)),
 verified_at INTEGER,
 status TEXT NOT NULL DEFAULT 'candidate',
 relay_id INTEGER REFERENCES relays(id) ON DELETE SET NULL,
 retire_at INTEGER,
 PRIMARY KEY(stored_key_id,url)
) STRICT;
CREATE INDEX key_relays_relay ON key_relays(relay_id);
CREATE TRIGGER key_relays_capacity BEFORE INSERT ON key_relays
WHEN NOT EXISTS(SELECT 1 FROM key_relays WHERE stored_key_id=new.stored_key_id AND url=new.url)
 AND (SELECT count(*) FROM key_relays WHERE stored_key_id=new.stored_key_id)>=16
BEGIN SELECT RAISE(ABORT, 'key relay capacity exceeded'); END;

-- Fresh instances use the qualified replacement. Never silently move existing grants.
UPDATE relays SET url='wss://bucket.coracle.social'
WHERE url='wss://relay.ditto.pub' AND NOT EXISTS(SELECT 1 FROM grants)
AND NOT EXISTS(SELECT 1 FROM relays WHERE url='wss://bucket.coracle.social');
