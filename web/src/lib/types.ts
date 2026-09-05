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
};

export type KeyWithRelations = {
    team: Team;
    stored_key: StoredKey;
    grants: Grant[];
};

export type GrantCreationResponse = {
    grant: Omit<Grant, "active_sessions" | "claimable_invitations">;
    bunker_uri: string;
};

export type InvitationCreationResponse = {
    invitation_id: number;
    bunker_uri: string;
};

export type SignerStatus = {
    resources: { inbox_records: number; inbox_bytes: number; pending_inputs: number; pending_responses: number; oldest_response_age_seconds: number; database_bytes: number; wal_bytes: number; last_backup_at: number | null; };
    ready: boolean;
    integrity_ok: boolean;
    recovery_pending: boolean;
    quarantined_grants: number;
    schema_version: number;
    envelope_version: number;
    credential_key_id: string | null;
    active_grants: number;
    active_sessions: number;
    claimable_invitations: number;
    enabled_relays: number;
    connected_relays: number;
    last_processed_at: number | null;
    ingress_rejections: number;
    parse_errors: number;
    denied_requests: number;
    relay_failures: number;
};

export type RelayStatus = {
    id: number;
    url: string;
    enabled: boolean;
    sort_order: number;
    last_connected_at: number | null;
    last_received_at: number | null;
    last_published_at: number | null;
    consecutive_failures: number;
    last_error: string | null;
};

export type StatusResponse = {
    signer: SignerStatus;
    database_ok: boolean;
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
