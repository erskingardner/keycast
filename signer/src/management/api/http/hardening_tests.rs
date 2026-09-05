// Included in the HTTP tests module so these exercise the complete production router/middleware.
fn signed_request(
    keys: &Keys,
    method: &str,
    path: &str,
    body: &str,
    revision: i64,
) -> Request<Body> {
    let header = auth_header(
        keys,
        method,
        &format!("http://example.com/api{path}"),
        body.as_bytes(),
    );
    let raw = BASE64
        .decode(header.strip_prefix("Nostr ").unwrap())
        .unwrap();
    let event = Event::from_json(raw).unwrap();
    let tags: Vec<_> = event
        .tags
        .iter()
        .map(|t| {
            if t.kind() == "revision" {
                Tag::parse(["revision", &revision.to_string()]).unwrap()
            } else {
                t.clone()
            }
        })
        .collect();
    let event = EventBuilder::new(event.kind, event.content)
        .tags(tags)
        .finalize(keys)
        .unwrap();
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", "example.com")
        .header("content-type", "application/json")
        .header(
            AUTHORIZATION_HEADER,
            format!("Nostr {}", BASE64.encode(event.as_json())),
        )
        .body(Body::from(body.to_owned()))
        .unwrap()
}
async fn route_request(
    app: &axum::Router,
    pool: &SqlitePool,
    keys: &Keys,
    method: &str,
    path: &str,
    body: &str,
) -> keycast_core::v2::control::HttpReply {
    let revision: i64 = query_scalar("SELECT authority_revision FROM instance_settings")
        .fetch_one(pool)
        .await
        .unwrap();
    let response = app
        .clone()
        .oneshot(signed_request(keys, method, path, body, revision))
        .await
        .unwrap();
    if method != "GET" && response.status() == StatusCode::OK {
        decrypt_reply(response, keys).await
    } else {
        keycast_core::v2::control::HttpReply {
            status: response.status().as_u16(),
            body: String::from_utf8(
                to_bytes(response.into_body(), MAX_AUTH_BODY_BYTES)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap(),
        }
    }
}
async fn member_row(pool: &SqlitePool, team: i64, keys: &Keys, role: &str) {
    query("INSERT INTO users(public_key) VALUES(?) ON CONFLICT(public_key) DO NOTHING")
        .bind(keys.public_key().to_hex())
        .execute(pool)
        .await
        .unwrap();
    query("INSERT INTO team_members(team_id,user_public_key,role) VALUES(?,?,?)")
        .bind(team)
        .bind(keys.public_key().to_hex())
        .bind(role)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn authorization_matrix_covers_all_team_resources_and_cross_team_ids() {
    let _guard = ENV_LOCK.lock().unwrap();
    let pool = setup_route_test_db().await;
    let admin = Keys::generate();
    let member = Keys::generate();
    let outsider = Keys::generate();
    env::set_var(
        "ALLOWED_PUBKEYS",
        format!(
            "{},{},{}",
            admin.public_key(),
            member.public_key(),
            outsider.public_key()
        ),
    );
    env::remove_var("KEYCAST_PUBLIC_URL");
    for name in ["one", "two"] {
        query("INSERT INTO teams(name) VALUES(?)")
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }
    member_row(&pool, 1, &admin, "admin").await;
    member_row(&pool, 1, &member, "member").await;
    member_row(&pool, 2, &outsider, "admin").await;
    let state = state(pool.clone());
    let store = &state.signer.runtime.store;
    let managed = Keys::generate();
    let key = store
        .seal_stored_key(
            1,
            &admin.public_key().to_hex(),
            "test".into(),
            zeroize::Zeroizing::new(managed.secret_key().to_secret_hex()),
        )
        .await
        .unwrap();
    query("INSERT INTO policies(team_id,name,document) VALUES(1,'test',?)")
        .bind(r#"{"version":1,"capabilities":{"sign_event":{"allowed_kinds":[1]}}}"#)
        .execute(&pool)
        .await
        .unwrap();
    let (_, invitation, _) = store
        .create_grant(
            1,
            &admin.public_key().to_hex(),
            key.id,
            1,
            "test".into(),
            None,
            chrono::Utc::now().timestamp() + 300,
        )
        .await
        .unwrap();
    let app = routes::routes(state.clone());
    let policy = r#"{"name":"test","document":{"version":1,"capabilities":{"sign_event":{"allowed_kinds":[1]}}}}"#;
    let new_user =
        serde_json::json!({"user_public_key":Keys::generate().public_key(),"role":"admin"})
            .to_string();
    let new_grant=serde_json::json!({"name":"test","policy_id":1,"invitation_expires_at":chrono::Utc::now().timestamp()+300}).to_string();
    let new_invitation =
        serde_json::json!({"expires_at":chrono::Utc::now().timestamp()+300}).to_string();
    let cases = vec![
        ("PUT", "/teams/1".into(), r#"{"name":"tampered"}"#),
        ("DELETE", "/teams/1".into(), ""),
        ("POST", "/teams/1/users".into(), new_user.as_str()),
        (
            "DELETE",
            format!("/teams/1/users/{}", admin.public_key()),
            "",
        ),
        (
            "POST",
            "/teams/1/keys".into(),
            r#"{"name":"test","secret_key":"not-reached"}"#,
        ),
        ("GET", format!("/teams/1/keys/{}", key.public_key), ""),
        ("DELETE", format!("/teams/1/keys/{}", key.public_key), ""),
        ("POST", "/teams/1/policies".into(), policy),
        ("PUT", "/teams/1/policies/1".into(), policy),
        ("DELETE", "/teams/1/policies/1".into(), ""),
        (
            "POST",
            format!("/teams/1/keys/{}/grants", key.public_key),
            new_grant.as_str(),
        ),
        (
            "DELETE",
            format!("/teams/1/keys/{}/grants/1", key.public_key),
            "",
        ),
        (
            "POST",
            "/teams/1/grants/1/invitations".into(),
            new_invitation.as_str(),
        ),
        ("DELETE", format!("/teams/1/invitations/{invitation}"), ""),
        ("GET", "/teams/1/audit".into(), ""),
    ];
    for actor in [&member, &outsider] {
        for (method, path, body) in &cases {
            let reply = route_request(&app, &pool, actor, method, path, body).await;
            let expected = if actor.public_key() == member.public_key() && path.ends_with("/audit")
            {
                200
            } else {
                403
            };
            assert_eq!(reply.status, expected, "{method} {path}: {}", reply.body);
        }
    }
    // An administrator of team 2 cannot address team 1 resources under team 2's URL.
    for (method, path, body) in &cases[5..14] {
        if path.ends_with("/policies") {
            continue;
        } // creation is valid within their own team
        let path = path.replacen("/teams/1/", "/teams/2/", 1);
        let reply = route_request(&app, &pool, &outsider, method, &path, body).await;
        assert_eq!(
            reply.status, 404,
            "cross-team {method} {path}: {}",
            reply.body
        );
    }
    assert_eq!(
        route_request(&app, &pool, &member, "GET", "/teams/1", "")
            .await
            .status,
        200
    );
    assert_eq!(
        route_request(&app, &pool, &outsider, "GET", "/teams/1", "")
            .await
            .status,
        403
    );
    let list = route_request(&app, &pool, &outsider, "GET", "/teams", "").await;
    let list: serde_json::Value = serde_json::from_str(&list.body).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["team"]["id"], 2);
    assert_eq!(
        query_scalar::<_, i64>("SELECT count(*) FROM grants WHERE revoked_at IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    env::remove_var("ALLOWED_PUBKEYS");
}

#[tokio::test]
async fn concurrent_admin_removal_retains_an_administrator_and_requires_fresh_approval() {
    let _guard = ENV_LOCK.lock().unwrap();
    let pool = setup_route_test_db().await;
    let first = Keys::generate();
    let second = Keys::generate();
    env::set_var(
        "ALLOWED_PUBKEYS",
        format!("{},{}", first.public_key(), second.public_key()),
    );
    env::remove_var("KEYCAST_PUBLIC_URL");
    query("INSERT INTO teams(name) VALUES('test')")
        .execute(&pool)
        .await
        .unwrap();
    member_row(&pool, 1, &first, "admin").await;
    member_row(&pool, 1, &second, "admin").await;
    let app = routes::routes(state(pool.clone()));
    let a = app.clone().oneshot(signed_request(
        &first,
        "DELETE",
        &format!("/teams/1/users/{}", first.public_key()),
        "",
        0,
    ));
    let b = app.clone().oneshot(signed_request(
        &second,
        "DELETE",
        &format!("/teams/1/users/{}", second.public_key()),
        "",
        0,
    ));
    let (a, b) = tokio::join!(a, b);
    let mut statuses = vec![];
    for (response, keys) in [(a.unwrap(), &first), (b.unwrap(), &second)] {
        statuses.push(if response.status() == StatusCode::OK {
            decrypt_reply(response, keys).await.status
        } else {
            response.status().as_u16()
        });
    }
    statuses.sort();
    assert_eq!(statuses, vec![204, 409]);
    let remaining: String =
        query_scalar("SELECT user_public_key FROM team_members WHERE team_id=1 AND role='admin'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let keys = if remaining == first.public_key().to_hex() {
        &first
    } else {
        &second
    };
    assert_eq!(
        route_request(
            &app,
            &pool,
            keys,
            "DELETE",
            &format!("/teams/1/users/{remaining}"),
            ""
        )
        .await
        .status,
        400
    );
    assert_eq!(
        query_scalar::<_, i64>("SELECT count(*) FROM team_members WHERE role='admin'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    env::remove_var("ALLOWED_PUBKEYS");
}

#[tokio::test]
async fn policy_removal_retains_history_but_cannot_reactivate_or_reuse_authority() {
    let _guard = ENV_LOCK.lock().unwrap();
    let pool = setup_route_test_db().await;
    let admin = Keys::generate();
    env::set_var("ALLOWED_PUBKEYS", admin.public_key().to_hex());
    env::remove_var("KEYCAST_PUBLIC_URL");
    query("INSERT INTO teams(name) VALUES('test')")
        .execute(&pool)
        .await
        .unwrap();
    member_row(&pool, 1, &admin, "admin").await;
    let state = state(pool.clone());
    let store = state.signer.runtime.store.clone();
    let app = routes::routes(state);
    let key = store
        .seal_stored_key(
            1,
            &admin.public_key().to_hex(),
            "test".into(),
            zeroize::Zeroizing::new(Keys::generate().secret_key().to_secret_hex()),
        )
        .await
        .unwrap();
    let body = r#"{"name":"test","document":{"version":1,"capabilities":{"sign_event":{"allowed_kinds":[1]}}}}"#;
    assert_eq!(
        route_request(&app, &pool, &admin, "POST", "/teams/1/policies", body)
            .await
            .status,
        201
    );
    let (grant, _, _) = store
        .create_grant(
            1,
            &admin.public_key().to_hex(),
            key.id,
            1,
            "test".into(),
            None,
            chrono::Utc::now().timestamp() + 300,
        )
        .await
        .unwrap();
    assert_eq!(
        route_request(&app, &pool, &admin, "DELETE", "/teams/1/policies/1", "")
            .await
            .status,
        400
    );
    assert_eq!(
        route_request(
            &app,
            &pool,
            &admin,
            "DELETE",
            &format!("/teams/1/keys/{}/grants/{}", key.public_key, grant.id),
            ""
        )
        .await
        .status,
        204
    );
    assert_eq!(
        route_request(&app, &pool, &admin, "DELETE", "/teams/1/policies/1", "")
            .await
            .status,
        204
    );
    assert_eq!(
        query_scalar::<_, i64>("SELECT count(*) FROM policies WHERE deleted_at IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        query_scalar::<_, i64>("SELECT count(*) FROM grants")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        route_request(&app, &pool, &admin, "PUT", "/teams/1/policies/1", body)
            .await
            .status,
        404
    );
    assert!(store
        .create_grant(
            1,
            &admin.public_key().to_hex(),
            key.id,
            1,
            "bad".into(),
            None,
            chrono::Utc::now().timestamp() + 300
        )
        .await
        .is_err());
    // Reusing the display name creates a distinct policy identity.
    assert_eq!(
        route_request(&app, &pool, &admin, "POST", "/teams/1/policies", body)
            .await
            .status,
        201
    );
    let team = route_request(&app, &pool, &admin, "GET", "/teams/1", "").await;
    let team: serde_json::Value = serde_json::from_str(&team.body).unwrap();
    assert_eq!(team["policies"].as_array().unwrap().len(), 1);
    assert_eq!(team["policies"][0]["id"], 2);
    env::remove_var("ALLOWED_PUBKEYS");
}
