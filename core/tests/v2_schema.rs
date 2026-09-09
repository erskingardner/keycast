use sqlx::{query::query, query_scalar::query_scalar, raw_sql::raw_sql};
use sqlx_sqlite::{SqlitePool, SqlitePoolOptions};

const V2_SCHEMA: &str = include_str!("../../database/migrations/0001_initial.sql");
const PUBKEY_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PUBKEY_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const PUBKEY_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const PUBKEY_D: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

#[tokio::test]
async fn current_relay_defaults_preserve_existing_grants_and_custom_configuration() {
    for scenario in ["fresh", "existing_grant", "custom"] {
        let pool = setup_database().await;
        for migration in [
            include_str!("../../database/migrations/0002_team_slugs.sql"),
            include_str!("../../database/migrations/0003_relay_reliability.sql"),
            include_str!("../../database/migrations/0004_key_relay_discovery.sql"),
        ] {
            raw_sql(migration).execute(&pool).await.unwrap();
        }
        if scenario == "existing_grant" {
            let team = insert_team(&pool, "Existing team").await;
            let key = insert_stored_key(&pool, team, PUBKEY_A).await;
            let policy = insert_policy(&pool, team, "Policy").await;
            insert_grant(&pool, team, key, policy, PUBKEY_B)
                .await
                .unwrap();
        } else if scenario == "custom" {
            query("UPDATE relays SET url='wss://relay.example' WHERE url='wss://relay.primal.net'")
                .execute(&pool)
                .await
                .unwrap();
        }
        raw_sql(include_str!(
            "../../database/migrations/0005_default_signing_relays.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let urls: Vec<String> = query_scalar("SELECT url FROM relays ORDER BY sort_order")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(
            urls,
            [
                "wss://nos.lol",
                if scenario == "custom" {
                    "wss://relay.example"
                } else {
                    "wss://relay.primal.net"
                },
                if scenario == "fresh" {
                    "wss://relay.damus.io"
                } else {
                    "wss://bucket.coracle.social"
                },
            ],
            "{scenario}"
        );
    }
}

async fn setup_database() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect to in-memory sqlite");

    query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("enable foreign keys");
    raw_sql(V2_SCHEMA)
        .execute(&pool)
        .await
        .expect("apply v2 schema");

    pool
}

async fn insert_team(pool: &SqlitePool, name: &str) -> i64 {
    query_scalar("INSERT INTO teams(name) VALUES (?) RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await
        .expect("insert team")
}

async fn insert_stored_key(pool: &SqlitePool, team_id: i64, pubkey: &str) -> i64 {
    query_scalar(
        r#"
        INSERT INTO stored_keys(
            team_id, name, public_key, secret_envelope, envelope_version, key_encryption_key_id
        )
        VALUES (?, 'Signing key', ?, x'01', 1, 'local-v1')
        RETURNING id
        "#,
    )
    .bind(team_id)
    .bind(pubkey)
    .fetch_one(pool)
    .await
    .expect("insert stored key")
}

async fn insert_policy(pool: &SqlitePool, team_id: i64, name: &str) -> i64 {
    query_scalar(
        r#"
        INSERT INTO policies(team_id, name, document)
        VALUES (?, ?, '{"version":1,"capabilities":{}}')
        RETURNING id
        "#,
    )
    .bind(team_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("insert policy")
}

async fn insert_grant(
    pool: &SqlitePool,
    team_id: i64,
    stored_key_id: i64,
    policy_id: i64,
    remote_signer_public_key: &str,
) -> Result<i64, sqlx::Error> {
    query_scalar(
        r#"
        INSERT INTO grants(
            team_id,
            stored_key_id,
            policy_id,
            name,
            remote_signer_public_key,
            remote_signer_secret_envelope,
            envelope_version,
            key_encryption_key_id
        )
        VALUES (?, ?, ?, 'Phone', ?, x'02', 1, 'local-v1')
        RETURNING id
        "#,
    )
    .bind(team_id)
    .bind(stored_key_id)
    .bind(policy_id)
    .bind(remote_signer_public_key)
    .fetch_one(pool)
    .await
}

#[tokio::test]
async fn v2_schema_contains_only_the_new_authorization_model() {
    let pool = setup_database().await;
    let tables: Vec<String> = query_scalar(
        "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_all(&pool)
    .await
    .expect("list tables");

    for expected in [
        "stored_keys",
        "policies",
        "grants",
        "invitations",
        "sessions",
        "relays",
        "processed_requests",
        "audit_events",
    ] {
        assert!(
            tables.iter().any(|table| table == expected),
            "missing {expected}"
        );
    }

    assert!(!tables.iter().any(|table| table == "authorizations"));
    assert!(!tables.iter().any(|table| table == "user_authorizations"));
    assert!(!tables.iter().any(|table| table == "permissions"));
    assert!(!tables.iter().any(|table| table == "policy_permissions"));

    let schema_version: i64 = query_scalar("PRAGMA user_version")
        .fetch_one(&pool)
        .await
        .expect("read schema version");
    assert_eq!(schema_version, 2);

    let relays: Vec<String> =
        query_scalar("SELECT url FROM relays WHERE enabled = 1 ORDER BY sort_order")
            .fetch_all(&pool)
            .await
            .expect("list bootstrap relays");
    assert_eq!(
        relays,
        [
            "wss://nos.lol",
            "wss://relay.primal.net",
            "wss://relay.ditto.pub",
        ]
    );
}

#[tokio::test]
async fn grant_cannot_cross_team_key_or_policy_boundaries() {
    let pool = setup_database().await;
    let first_team = insert_team(&pool, "First").await;
    let second_team = insert_team(&pool, "Second").await;
    let first_key = insert_stored_key(&pool, first_team, PUBKEY_A).await;
    let second_policy = insert_policy(&pool, second_team, "Second policy").await;

    let error = insert_grant(&pool, first_team, first_key, second_policy, PUBKEY_C)
        .await
        .expect_err("cross-team policy must fail");
    assert!(matches!(error, sqlx::Error::Database(_)));

    let first_policy = insert_policy(&pool, first_team, "First policy").await;
    insert_grant(&pool, first_team, first_key, first_policy, PUBKEY_C)
        .await
        .expect("same-team grant succeeds");
}

#[tokio::test]
async fn invitation_secret_is_one_way_and_one_invitation_creates_one_session() {
    let pool = setup_database().await;
    let team = insert_team(&pool, "Family").await;
    let key = insert_stored_key(&pool, team, PUBKEY_A).await;
    let policy = insert_policy(&pool, team, "Notes").await;
    let grant = insert_grant(&pool, team, key, policy, PUBKEY_C)
        .await
        .expect("insert grant");
    let other_grant = insert_grant(&pool, team, key, policy, PUBKEY_D)
        .await
        .expect("insert other grant");

    let invitation: i64 = query_scalar(
        r#"
        INSERT INTO invitations(grant_id, secret_hash, expires_at)
        VALUES (?, zeroblob(32), unixepoch() + 300)
        RETURNING id
        "#,
    )
    .bind(grant)
    .fetch_one(&pool)
    .await
    .expect("insert invitation");

    query("UPDATE invitations SET consumed_at = unixepoch() WHERE id = ?")
        .bind(invitation)
        .execute(&pool)
        .await
        .expect("consume invitation");
    query(
        r#"
        INSERT INTO sessions(grant_id, invitation_id, client_public_key)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(grant)
    .bind(invitation)
    .bind(PUBKEY_B)
    .execute(&pool)
    .await
    .expect("create session");

    let duplicate_invitation = query(
        r#"
        INSERT INTO sessions(grant_id, invitation_id, client_public_key)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(grant)
    .bind(invitation)
    .bind(PUBKEY_A)
    .execute(&pool)
    .await;
    assert!(duplicate_invitation.is_err());

    let duplicate_active_client: i64 = query_scalar(
        r#"
        INSERT INTO invitations(grant_id, secret_hash, expires_at)
        VALUES (?, randomblob(32), unixepoch() + 300)
        RETURNING id
        "#,
    )
    .bind(grant)
    .fetch_one(&pool)
    .await
    .expect("insert second invitation");
    let duplicate_active_client = query(
        r#"
        INSERT INTO sessions(grant_id, invitation_id, client_public_key)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(grant)
    .bind(duplicate_active_client)
    .bind(PUBKEY_B)
    .execute(&pool)
    .await;
    assert!(duplicate_active_client.is_err());

    let other_grant_invitation: i64 = query_scalar(
        r#"
        INSERT INTO invitations(grant_id, secret_hash, expires_at)
        VALUES (?, randomblob(32), unixepoch() + 300)
        RETURNING id
        "#,
    )
    .bind(grant)
    .fetch_one(&pool)
    .await
    .expect("insert invitation for cross-grant check");
    let cross_grant_session = query(
        r#"
        INSERT INTO sessions(grant_id, invitation_id, client_public_key)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(other_grant)
    .bind(other_grant_invitation)
    .bind(PUBKEY_A)
    .execute(&pool)
    .await;
    assert!(cross_grant_session.is_err());
}

#[tokio::test]
async fn policies_require_json_objects_and_never_implicit_legacy_permissions() {
    let pool = setup_database().await;
    let team = insert_team(&pool, "Team").await;

    for invalid in ["not-json", "[]", "null", "true"] {
        let result = query("INSERT INTO policies(team_id, name, document) VALUES (?, ?, ?)")
            .bind(team)
            .bind(format!("Policy {invalid}"))
            .bind(invalid)
            .execute(&pool)
            .await;
        assert!(
            result.is_err(),
            "accepted invalid policy document {invalid}"
        );
    }

    insert_policy(&pool, team, "Explicit capabilities").await;
}

#[tokio::test]
async fn deleting_a_stored_key_cascades_runtime_authorization_state() {
    let pool = setup_database().await;
    let team = insert_team(&pool, "Team").await;
    let key = insert_stored_key(&pool, team, PUBKEY_A).await;
    let policy = insert_policy(&pool, team, "Policy").await;
    let grant = insert_grant(&pool, team, key, policy, PUBKEY_C)
        .await
        .expect("insert grant");
    query(
        "INSERT INTO invitations(grant_id, secret_hash, expires_at) VALUES (?, zeroblob(32), unixepoch() + 300)",
    )
    .bind(grant)
    .execute(&pool)
    .await
    .expect("insert invitation");

    query("UPDATE invitations SET consumed_at=unixepoch()")
        .execute(&pool)
        .await
        .unwrap();
    query("INSERT INTO sessions(grant_id,invitation_id,client_public_key) SELECT grant_id,id,? FROM invitations")
        .bind(PUBKEY_B).execute(&pool).await.unwrap();
    query("DELETE FROM stored_keys WHERE id = ?")
        .bind(key)
        .execute(&pool)
        .await
        .expect("delete stored key");

    let grants: i64 = query_scalar("SELECT COUNT(*) FROM grants")
        .fetch_one(&pool)
        .await
        .expect("count grants");
    let invitations: i64 = query_scalar("SELECT COUNT(*) FROM invitations")
        .fetch_one(&pool)
        .await
        .expect("count invitations");
    assert_eq!(
        query_scalar::<_, i64>("SELECT count(*) FROM sessions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(grants, 0);
    assert_eq!(invitations, 0);
}
