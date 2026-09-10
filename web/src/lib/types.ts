export type StoredKey = {
    id: number;
    name: string;
    team_id: number;
    public_key: string;
    created_at: number;
    updated_at: number;
};

export type User = {
    user_public_key: string;
    role: "admin" | "member";
    created_at: number;
};

export type Team = {
    id: number;
    name: string;
    slug: string | null;
    created_at: number;
    updated_at: number;
};

export type RecipientScope = "self_only" | "any";

export type CryptoCapability = {
    recipient: RecipientScope;
};

export type PolicyDocument = {
    version: 1;
    capabilities: {
        sign_event?: { allowed_kinds: number[] };
        nip04_encrypt?: CryptoCapability;
        nip04_decrypt?: CryptoCapability;
        nip44_encrypt?: CryptoCapability;
        nip44_decrypt?: CryptoCapability;
    };
};

export type Policy = {
    id: number;
    team_id: number;
    name: string;
    document: PolicyDocument;
    revision: number;
    created_at: number;
    updated_at: number;
};

export type TeamWithRelations = {
    team: Team;
    team_users: User[];
    stored_keys: StoredKey[];
    policies: Policy[];
};

export type Grant = {
    id: number;
    team_id: number;
    stored_key_id: number;
    policy_id: number;
    name: string;
    remote_signer_public_key: string;
    expires_at: number | null;
    revoked_at: number | null;
    created_at: number;
    updated_at: number;
    active_sessions: number;
    claimable_invitations: number;
    invitations: { id: number; expires_at: number }[];
};

export type KeyRelayInfo = {
    event_id: string | null; created_at: number | null; fetched_at: number | null;
    status: string; auto_activate: boolean;
    relays: { url: string; read: boolean; write: boolean; listed: boolean; status: string; verified_at: number | null; active: boolean }[];
};
export type KeyWithRelations = {
    relay_discovery: KeyRelayInfo;
    team: Team;
    stored_key: StoredKey;
    grants: Grant[];
};

export type GrantCreationResponse = {
    grant: Omit<Grant, "active_sessions" | "claimable_invitations" | "invitations">;
    bunker_uri: string;
};

export type InvitationCreationResponse = {
    invitation_id: number;
    bunker_uri: string;
};

export type SignerStatus = {
    software_version?: string;
    build_revision?: string;
    resources: { inbox_records: number; inbox_bytes: number; pending_inputs: number; pending_responses: number; oldest_response_age_seconds: number; database_bytes: number; wal_bytes: number; last_backup_at: number | null; };
    ready: boolean;
    integrity_ok: boolean;
    recovery_pending: boolean;
    quarantined_grants: number;
    schema_version: number;
    envelope_version: number;
    credential_key_id: string | null;
    management_reply_public_key: string | null;
    active_grants: number;
    active_sessions: number;
    claimable_invitations: number;
    invitations: { id: number; expires_at: number }[];
    enabled_relays: number;
    connected_relays: number;
    last_processed_at: number | null;
    ingress_rejections: number;
    cached_retries: number;
    retry_coalesced: number;
    retry_throttled: number;
    storage_rejections: number;
    parse_errors: number;
    denied_requests: number;
    relay_failures: number;
};

export type RelayStatus = {
    discovered: boolean;
    id: number;
    url: string;
    enabled: boolean;
    sort_order: number;
    last_connected_at: number | null;
    last_received_at: number | null;
    last_published_at: number | null;
    consecutive_failures: number;
    last_error: string | null;
    diagnostics: RelayDiagnostics | null;
    reliability: RelayReliability | null;
};

export type RelayReliabilityCounts = {
    attempts: number;
    retries: number;
    connections: number;
    remote_closes: number;
    connection_losses: number;
    errors: number;
    cancelled_attempts: number;
    dropped_observations: number;
};
export type RelayReliability = {
    tracking_since: number | null;
    window_start: number;
    lifetime: RelayReliabilityCounts;
    last_7_days: RelayReliabilityCounts;
    categories: { category: string; message: string; count: number }[];
    history: { category: string; message: string; count: number; first_at: number; last_at: number }[];
};

export type RelayDiagnostics = {
    connection: string;
    subscription: "idle" | "pending" | "accepted" | "rejected";
    observed_at: number;
    connected_at: number | null;
    attempts: number;
    successes: number;
    bytes_sent: number;
    bytes_received: number;
    latency_ms: number | null;
    subscription_error: string | null;
    transport_error: string | null;
    history: { occurred_at: number; level: "info" | "warning"; message: string }[];
};

export type StatusResponse = {
    signer: SignerStatus;
    database_ok: boolean;
    auto_activate_relays: boolean;
    minimum_connected_relays: number;
    relays: RelayStatus[];
};

export type AuditEvent = {
    id: number;
    occurred_at: number;
    grant_id: number | null;
    session_id: number | null;
    actor_public_key: string | null;
    action: string;
    outcome: "allowed" | "denied" | "succeeded" | "failed";
    reason_code: string | null;
    request_event_id: string | null;
};
