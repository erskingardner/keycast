PRAGMA foreign_keys = ON;

-- Keycast v2 is a clean-slate schema. Legacy authorization and redemption tables are intentionally
-- absent: old bunker URLs and their reusable connection secrets are not valid in v2.

CREATE TABLE users (
    public_key TEXT PRIMARY KEY
        CHECK (
            length(public_key) = 64
            AND public_key = lower(public_key)
            AND public_key NOT GLOB '*[^0-9a-f]*'
        ),
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE TABLE teams (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 120),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE TABLE team_members (
    team_id INTEGER NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_public_key TEXT NOT NULL REFERENCES users(public_key) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('admin', 'member')),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (team_id, user_public_key)
) STRICT;

CREATE INDEX team_members_user_public_key_idx ON team_members(user_public_key);

CREATE TABLE stored_keys (
    id INTEGER PRIMARY KEY,
    team_id INTEGER NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 120),
    public_key TEXT NOT NULL
        CHECK (
            length(public_key) = 64
            AND public_key = lower(public_key)
            AND public_key NOT GLOB '*[^0-9a-f]*'
        ),
    secret_envelope BLOB NOT NULL CHECK (length(secret_envelope) > 0),
    envelope_version INTEGER NOT NULL CHECK (envelope_version >= 1),
    key_encryption_key_id TEXT NOT NULL CHECK (length(trim(key_encryption_key_id)) > 0),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE (team_id, public_key),
    UNIQUE (id, team_id)
) STRICT;

CREATE INDEX stored_keys_team_id_idx ON stored_keys(team_id);

-- Policy documents are strict, versioned Rust types. The database verifies only that the stored
-- representation is a JSON object; semantic validation belongs at every API and signer boundary.
-- Missing capabilities always mean deny.
CREATE TABLE policies (
    id INTEGER PRIMARY KEY,
    team_id INTEGER NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 120),
    document TEXT NOT NULL
        CHECK (json_valid(document) AND json_type(document) = 'object'),
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at INTEGER,
    UNIQUE (id, team_id)
) STRICT;

CREATE INDEX policies_team_id_idx ON policies(team_id);
CREATE UNIQUE INDEX policies_active_name_idx ON policies(team_id,name) WHERE deleted_at IS NULL;

-- A grant is the durable association between a stored key and a live policy. Each grant keeps a
-- distinct NIP-46 communication key so it can be independently routed and revoked.
CREATE TABLE grants (
    id INTEGER PRIMARY KEY,
    team_id INTEGER NOT NULL,
    stored_key_id INTEGER NOT NULL,
    policy_id INTEGER NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 120),
    remote_signer_public_key TEXT NOT NULL UNIQUE
        CHECK (
            length(remote_signer_public_key) = 64
            AND remote_signer_public_key = lower(remote_signer_public_key)
            AND remote_signer_public_key NOT GLOB '*[^0-9a-f]*'
        ),
    remote_signer_secret_envelope BLOB NOT NULL
        CHECK (length(remote_signer_secret_envelope) > 0),
    envelope_version INTEGER NOT NULL CHECK (envelope_version >= 1),
    key_encryption_key_id TEXT NOT NULL CHECK (length(trim(key_encryption_key_id)) > 0),
    expires_at INTEGER,
    revoked_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (stored_key_id, team_id)
        REFERENCES stored_keys(id, team_id) ON DELETE CASCADE,
    FOREIGN KEY (policy_id, team_id)
        REFERENCES policies(id, team_id) ON DELETE RESTRICT,
    CHECK (expires_at IS NULL OR expires_at > created_at),
    CHECK (revoked_at IS NULL OR revoked_at >= created_at)
) STRICT;

CREATE INDEX grants_team_id_idx ON grants(team_id);
CREATE INDEX grants_stored_key_id_idx ON grants(stored_key_id);
CREATE INDEX grants_policy_id_idx ON grants(policy_id);
CREATE INDEX grants_active_idx ON grants(remote_signer_public_key)
    WHERE revoked_at IS NULL;

-- Invitation secrets are generated with 256 bits of randomness and shown exactly once. Only their
-- SHA-256 hashes are retained. Claiming an invitation and creating its session is one transaction.
CREATE TABLE invitations (
    id INTEGER PRIMARY KEY,
    grant_id INTEGER NOT NULL REFERENCES grants(id) ON DELETE CASCADE,
    secret_hash BLOB NOT NULL UNIQUE CHECK (length(secret_hash) = 32),
    expires_at INTEGER NOT NULL,
    consumed_at INTEGER,
    revoked_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE (id, grant_id),
    CHECK (expires_at > created_at),
    CHECK (consumed_at IS NULL OR consumed_at >= created_at),
    CHECK (revoked_at IS NULL OR revoked_at >= created_at),
    CHECK (NOT (consumed_at IS NOT NULL AND revoked_at IS NOT NULL))
) STRICT;

CREATE INDEX invitations_claimable_idx ON invitations(grant_id, expires_at)
    WHERE consumed_at IS NULL AND revoked_at IS NULL;

-- requested_capabilities is a normalized upper bound requested by the client. The effective
-- permissions are its intersection with the grant's current policy, so later policy changes apply
-- immediately and can never be widened by the client.
CREATE TABLE sessions (
    id INTEGER PRIMARY KEY,
    grant_id INTEGER NOT NULL REFERENCES grants(id) ON DELETE CASCADE,
    invitation_id INTEGER NOT NULL UNIQUE,
    client_public_key TEXT NOT NULL
        CHECK (
            length(client_public_key) = 64
            AND client_public_key = lower(client_public_key)
            AND client_public_key NOT GLOB '*[^0-9a-f]*'
        ),
    requested_capabilities TEXT
        CHECK (
            requested_capabilities IS NULL
            OR (json_valid(requested_capabilities) AND json_type(requested_capabilities) = 'array')
        ),
    client_metadata TEXT
        CHECK (
            client_metadata IS NULL
            OR (json_valid(client_metadata) AND json_type(client_metadata) = 'object')
        ),
    connected_at INTEGER NOT NULL DEFAULT (unixepoch()),
    last_seen_at INTEGER NOT NULL DEFAULT (unixepoch()),
    ended_at INTEGER,
    end_reason TEXT CHECK (
        end_reason IS NULL OR end_reason IN ('logout', 'revoked', 'expired', 'replaced')
    ),
    CHECK (
        (ended_at IS NULL AND end_reason IS NULL)
        OR (ended_at IS NOT NULL AND end_reason IS NOT NULL AND ended_at >= connected_at)
    ),
    UNIQUE (id, grant_id),
    FOREIGN KEY (invitation_id, grant_id)
        REFERENCES invitations(id, grant_id) ON DELETE NO ACTION
) STRICT;

CREATE UNIQUE INDEX sessions_one_active_client_per_grant_idx
    ON sessions(grant_id, client_public_key)
    WHERE ended_at IS NULL;
CREATE INDEX sessions_active_grant_idx ON sessions(grant_id)
    WHERE ended_at IS NULL;

-- Relay configuration is instance-wide. The signer opens one shared connection to every enabled
-- relay and advertises the ordered set through bunker invitations and switch_relays responses.
CREATE TABLE relays (
    id INTEGER PRIMARY KEY,
    url TEXT NOT NULL UNIQUE CHECK (length(trim(url)) > 0),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE INDEX relays_enabled_order_idx ON relays(enabled, sort_order, id);

-- A fresh self-hosted instance must be able to issue a usable bunker URI without a hidden
-- provisioning step. These are ordinary public relays and remain editable through the API.
INSERT INTO relays(url, sort_order) VALUES
    ('wss://nos.lol', 10),
    ('wss://relay.primal.net', 20),
    ('wss://relay.ditto.pub', 30);

CREATE TABLE relay_checkpoints (
    relay_id INTEGER PRIMARY KEY REFERENCES relays(id) ON DELETE CASCADE,
    last_event_created_at INTEGER,
    last_event_id TEXT CHECK (
        last_event_id IS NULL
        OR (
            length(last_event_id) = 64
            AND last_event_id = lower(last_event_id)
            AND last_event_id NOT GLOB '*[^0-9a-f]*'
        )
    ),
    last_connected_at INTEGER,
    last_received_at INTEGER,
    last_published_at INTEGER,
    consecutive_failures INTEGER NOT NULL DEFAULT 0 CHECK (consecutive_failures >= 0),
    last_error TEXT,
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

-- One row per request event supplies cross-relay deduplication and an idempotent response cache.
-- response_event_json contains an already encrypted and signed NIP-46 response, never plaintext.
CREATE TABLE processed_requests (
    event_id TEXT PRIMARY KEY
        CHECK (
            length(event_id) = 64
            AND event_id = lower(event_id)
            AND event_id NOT GLOB '*[^0-9a-f]*'
        ),
    grant_id INTEGER NOT NULL REFERENCES grants(id) ON DELETE CASCADE,
    session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
    client_public_key TEXT NOT NULL
        CHECK (
            length(client_public_key) = 64
            AND client_public_key = lower(client_public_key)
            AND client_public_key NOT GLOB '*[^0-9a-f]*'
        ),
    request_id TEXT NOT NULL CHECK (length(request_id) BETWEEN 1 AND 256),
    method TEXT NOT NULL CHECK (length(method) BETWEEN 1 AND 64),
    status TEXT NOT NULL CHECK (
        status IN ('processing', 'approved', 'denied', 'completed', 'failed')
    ),
    reason_code TEXT,
    response_event_json TEXT CHECK (
        response_event_json IS NULL OR json_valid(response_event_json)
    ),
    request_event_json TEXT,
    reserved_response_bytes INTEGER NOT NULL DEFAULT 0 CHECK(reserved_response_bytes BETWEEN 0 AND 270336),
    publish_attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at INTEGER NOT NULL DEFAULT 0,
    received_at INTEGER NOT NULL DEFAULT (unixepoch()),
    completed_at INTEGER,
    CHECK (completed_at IS NULL OR completed_at >= received_at)
) STRICT;

CREATE INDEX processed_requests_grant_received_idx
    ON processed_requests(grant_id, received_at);
CREATE INDEX processed_requests_session_received_idx
    ON processed_requests(session_id, received_at);

-- Audit details must contain identifiers and non-sensitive diagnostics only. Secrets, plaintext,
-- ciphertext, private keys, and complete bunker URLs are forbidden at the application boundary.
CREATE TABLE audit_events (
    id INTEGER PRIMARY KEY,
    occurred_at INTEGER NOT NULL DEFAULT (unixepoch()),
    team_id INTEGER REFERENCES teams(id) ON DELETE SET NULL,
    stored_key_id INTEGER REFERENCES stored_keys(id) ON DELETE SET NULL,
    grant_id INTEGER REFERENCES grants(id) ON DELETE SET NULL,
    session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
    actor_public_key TEXT CHECK (
        actor_public_key IS NULL
        OR (
            length(actor_public_key) = 64
            AND actor_public_key = lower(actor_public_key)
            AND actor_public_key NOT GLOB '*[^0-9a-f]*'
        )
    ),
    action TEXT NOT NULL CHECK (length(action) BETWEEN 1 AND 80),
    outcome TEXT NOT NULL CHECK (outcome IN ('allowed', 'denied', 'succeeded', 'failed')),
    reason_code TEXT,
    request_event_id TEXT,
    details TEXT NOT NULL DEFAULT '{}'
        CHECK (json_valid(details) AND json_type(details) = 'object')
) STRICT;

CREATE INDEX audit_events_occurred_at_idx ON audit_events(occurred_at);
CREATE INDEX audit_events_team_occurred_idx ON audit_events(team_id, occurred_at);
CREATE INDEX audit_events_grant_occurred_idx ON audit_events(grant_id, occurred_at);

CREATE TABLE instance_settings (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    instance_id TEXT NOT NULL DEFAULT (lower(hex(randomblob(32)))),
    authority_revision INTEGER NOT NULL DEFAULT 0,
    root_key_id TEXT,
    last_backup_at INTEGER,
    recovery_pending INTEGER NOT NULL DEFAULT 0 CHECK (recovery_pending IN (0,1)),
    minimum_connected_relays INTEGER NOT NULL DEFAULT 1
        CHECK (minimum_connected_relays >= 1),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

INSERT INTO instance_settings(singleton) VALUES (1);

PRAGMA user_version = 2;

-- External management approvals carry a unique nonce; even rejected commands consume it.
CREATE TABLE management_nonces (
    nonce TEXT PRIMARY KEY,
    actor_public_key TEXT NOT NULL,
    accepted_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;
CREATE INDEX management_nonces_age ON management_nonces(accepted_at);

CREATE INDEX processed_requests_outbox ON processed_requests(status,next_attempt_at);
CREATE INDEX processed_requests_age ON processed_requests(received_at);
CREATE INDEX processed_requests_completion ON processed_requests(completed_at) WHERE status='completed';
CREATE TABLE request_storage_budget (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    bytes INTEGER NOT NULL DEFAULT 0 CHECK(bytes BETWEEN 0 AND 134217728),
    records INTEGER NOT NULL DEFAULT 0 CHECK(records BETWEEN 0 AND 10000)
) STRICT;
INSERT INTO request_storage_budget(singleton) VALUES(1);
CREATE TRIGGER request_budget_insert AFTER INSERT ON processed_requests BEGIN
    UPDATE request_storage_budget SET bytes=bytes+coalesce(length(CAST(new.request_event_json AS BLOB)),0)+coalesce(length(CAST(new.response_event_json AS BLOB)),0)+new.reserved_response_bytes, records=records+1;
END;
CREATE TRIGGER request_budget_update AFTER UPDATE OF request_event_json,response_event_json,reserved_response_bytes ON processed_requests BEGIN
    UPDATE request_storage_budget SET bytes=bytes+coalesce(length(CAST(new.request_event_json AS BLOB)),0)+coalesce(length(CAST(new.response_event_json AS BLOB)),0)+new.reserved_response_bytes-coalesce(length(CAST(old.request_event_json AS BLOB)),0)-coalesce(length(CAST(old.response_event_json AS BLOB)),0)-old.reserved_response_bytes;
END;
CREATE TRIGGER request_budget_delete AFTER DELETE ON processed_requests BEGIN
    UPDATE request_storage_budget SET bytes=bytes-coalesce(length(CAST(old.request_event_json AS BLOB)),0)-coalesce(length(CAST(old.response_event_json AS BLOB)),0)-old.reserved_response_bytes,records=records-1;
END;
CREATE TRIGGER audit_volume AFTER INSERT ON audit_events BEGIN
    DELETE FROM audit_events WHERE id <= new.id-100000;
END;

-- Bound administrative storage independently of request admission.
CREATE TRIGGER users_capacity BEFORE INSERT ON users WHEN NOT EXISTS(SELECT 1 FROM users WHERE public_key=new.public_key) AND (SELECT count(*) FROM users) >= 1000 BEGIN SELECT RAISE(ABORT, 'instance capacity exceeded'); END;
CREATE TRIGGER teams_capacity BEFORE INSERT ON teams WHEN (SELECT count(*) FROM teams) >= 1000 BEGIN SELECT RAISE(ABORT, 'instance capacity exceeded'); END;
CREATE TRIGGER team_members_capacity BEFORE INSERT ON team_members WHEN (SELECT count(*) FROM team_members) >= 10000 BEGIN SELECT RAISE(ABORT, 'instance capacity exceeded'); END;
CREATE TRIGGER stored_keys_capacity BEFORE INSERT ON stored_keys WHEN (SELECT count(*) FROM stored_keys) >= 1000 BEGIN SELECT RAISE(ABORT, 'instance capacity exceeded'); END;
CREATE TRIGGER policies_capacity BEFORE INSERT ON policies WHEN (SELECT count(*) FROM policies) >= 1000 BEGIN SELECT RAISE(ABORT, 'instance capacity exceeded'); END;
CREATE TRIGGER grants_capacity BEFORE INSERT ON grants WHEN (SELECT count(*) FROM grants) >= 1000 BEGIN SELECT RAISE(ABORT, 'instance capacity exceeded'); END;
CREATE TRIGGER invitations_capacity BEFORE INSERT ON invitations WHEN (SELECT count(*) FROM invitations) >= 10000 BEGIN SELECT RAISE(ABORT, 'instance capacity exceeded'); END;
CREATE TRIGGER sessions_capacity BEFORE INSERT ON sessions WHEN (SELECT count(*) FROM sessions) >= 10000 BEGIN SELECT RAISE(ABORT, 'instance capacity exceeded'); END;
CREATE TRIGGER relays_capacity BEFORE INSERT ON relays WHEN NOT EXISTS(SELECT 1 FROM relays WHERE url=new.url) AND (SELECT count(*) FROM relays) >= 20 BEGIN SELECT RAISE(ABORT, 'instance capacity exceeded'); END;
CREATE TRIGGER management_nonces_capacity BEFORE INSERT ON management_nonces WHEN (SELECT count(*) FROM management_nonces) >= 10000 BEGIN SELECT RAISE(ABORT, 'instance capacity exceeded'); END;

CREATE INDEX management_nonces_actor ON management_nonces(actor_public_key);
CREATE TRIGGER management_nonce_actor_capacity BEFORE INSERT ON management_nonces WHEN (SELECT count(*) FROM management_nonces WHERE actor_public_key=new.actor_public_key) >= 128 BEGIN SELECT RAISE(ABORT, 'actor approval capacity exceeded'); END;

-- Reserve a bounded encrypted reply before executing any request. A noisy grant/client/team
-- cannot consume the shared inbox, and completing an admitted request needs no extra budget.
CREATE INDEX processed_requests_client ON processed_requests(client_public_key);
CREATE TRIGGER request_capacity BEFORE INSERT ON processed_requests
WHEN NOT EXISTS(SELECT 1 FROM processed_requests WHERE event_id=new.event_id) BEGIN
    SELECT CASE WHEN (SELECT records>=10000 OR bytes+coalesce(length(CAST(new.request_event_json AS BLOB)),0)+new.reserved_response_bytes>134217728 FROM request_storage_budget)
        THEN RAISE(ABORT, 'request capacity exceeded') END;
    SELECT CASE WHEN (SELECT count(*)>=2000 OR coalesce(sum(coalesce(length(CAST(request_event_json AS BLOB)),0)+coalesce(length(CAST(response_event_json AS BLOB)),0)+reserved_response_bytes),0)+coalesce(length(CAST(new.request_event_json AS BLOB)),0)+new.reserved_response_bytes>4194304 FROM processed_requests WHERE grant_id=new.grant_id)
        THEN RAISE(ABORT, 'grant request capacity exceeded') END;
    SELECT CASE WHEN (SELECT count(*)>=2500 OR coalesce(sum(coalesce(length(CAST(request_event_json AS BLOB)),0)+coalesce(length(CAST(response_event_json AS BLOB)),0)+reserved_response_bytes),0)+coalesce(length(CAST(new.request_event_json AS BLOB)),0)+new.reserved_response_bytes>8388608 FROM processed_requests WHERE client_public_key=new.client_public_key)
        THEN RAISE(ABORT, 'client request capacity exceeded') END;
    SELECT CASE WHEN (SELECT count(*)>=4000 OR coalesce(sum(coalesce(length(CAST(r.request_event_json AS BLOB)),0)+coalesce(length(CAST(r.response_event_json AS BLOB)),0)+r.reserved_response_bytes),0)+coalesce(length(CAST(new.request_event_json AS BLOB)),0)+new.reserved_response_bytes>16777216 FROM processed_requests r JOIN grants g ON g.id=r.grant_id WHERE g.team_id=(SELECT team_id FROM grants WHERE id=new.grant_id))
        THEN RAISE(ABORT, 'team request capacity exceeded') END;
END;
