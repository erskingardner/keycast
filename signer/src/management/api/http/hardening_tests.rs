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

#[tokio::test]
async fn rejected_commands_cannot_invalidate_another_actors_approval_or_fill_the_nonce_pool() {
    let _guard = ENV_LOCK.lock().unwrap();
    let pool = setup_route_test_db().await;
    let admin = Keys::generate();
    let outsider = Keys::generate();
    env::set_var("ALLOWED_PUBKEYS",format!("{},{}",admin.public_key(),outsider.public_key()));
    let app = routes::routes(state(pool.clone()));
    let approved = signed_request(&admin,"POST","/teams",r#"{"name":"Approved"}"#,0);
    for _ in 0..128 {
        assert_eq!(route_request(&app,&pool,&outsider,"DELETE","/teams/999999","").await.status,403);
    }
    assert_eq!(route_request(&app,&pool,&outsider,"DELETE","/teams/999999","").await.status,409);
    assert_eq!(query_scalar::<_,i64>("SELECT authority_revision FROM instance_settings").fetch_one(&pool).await.unwrap(),0);
    assert_eq!(decrypt_reply(app.oneshot(approved).await.unwrap(),&admin).await.status,201);
    assert_eq!(query_scalar::<_,i64>("SELECT authority_revision FROM instance_settings").fetch_one(&pool).await.unwrap(),1);
    env::remove_var("ALLOWED_PUBKEYS");
}

#[tokio::test]
async fn management_reads_reject_delegated_http_auth_wrong_instances_and_nonoperator_status() {
    let _guard = ENV_LOCK.lock().unwrap();
    let pool = setup_route_test_db().await;
    let keys = Keys::generate();
    env::set_var("ALLOWED_PUBKEYS",keys.public_key().to_hex());
    env::set_var("KEYCAST_OPERATOR_PUBKEYS",keys.public_key().to_hex().to_uppercase());
    let app = routes::routes(state(pool.clone()));
    for (kind,instance,expected) in [(27235,"test-instance",401),(27237,"another-instance",401),(27237,"test-instance",200)] {
        let event=EventBuilder::new(Kind::Custom(kind),"").tags([
            Tag::parse(["u","http://example.com/api/status"]).unwrap(),
            Tag::parse(["method","GET"]).unwrap(),
            Tag::parse(["instance",instance]).unwrap(),
        ]).finalize(&keys).unwrap();
        let request=Request::builder().uri("/status").header("host","attacker.example")
            .header(AUTHORIZATION_HEADER,format!("Nostr {}",BASE64.encode(event.as_json()))).body(Body::empty()).unwrap();
        assert_eq!(app.clone().oneshot(request).await.unwrap().status().as_u16(),expected);
    }
    env::remove_var("KEYCAST_OPERATOR_PUBKEYS");
    assert_eq!(route_request(&app,&pool,&keys,"GET","/status","").await.status,403);
    env::remove_var("ALLOWED_PUBKEYS");
}

#[tokio::test]
async fn relay_replacement_at_capacity_preserves_disabled_rows_and_existing_checkpoints() {
    let _guard = ENV_LOCK.lock().unwrap();
    let pool=setup_route_test_db().await;
    let keys=Keys::generate();
    env::set_var("ALLOWED_PUBKEYS",keys.public_key().to_hex());
    env::set_var("KEYCAST_OPERATOR_PUBKEYS",keys.public_key().to_hex());
    let app=routes::routes(state(pool.clone()));
    let relays:Vec<_>=(0..20).map(|n|serde_json::json!({"url":format!("wss://relay{n}.example"),"enabled":n!=19})).collect();
    let request=serde_json::json!({"minimum_connected_relays":1,"relays":relays}).to_string();
    assert_eq!(route_request(&app,&pool,&keys,"PUT","/relays",&request).await.status,200);
    query("INSERT INTO relay_checkpoints(relay_id,last_connected_at) SELECT id,123 FROM relays").execute(&pool).await.unwrap();
    assert_eq!(route_request(&app,&pool,&keys,"PUT","/relays",&request).await.status,200);
    assert_eq!(query_scalar::<_,i64>("SELECT count(*) FROM relay_checkpoints WHERE last_connected_at=123").fetch_one(&pool).await.unwrap(),20);
    assert_eq!(query_scalar::<_,i64>("SELECT count(*) FROM relays WHERE enabled=0").fetch_one(&pool).await.unwrap(),1);
    let request=serde_json::json!({"minimum_connected_relays":1,"relays":[{"url":"wss://replacement.example","enabled":true}]}).to_string();
    assert_eq!(route_request(&app,&pool,&keys,"PUT","/relays",&request).await.status,200);
    assert_eq!(query_scalar::<_,i64>("SELECT count(*) FROM relays").fetch_one(&pool).await.unwrap(),1);
    env::remove_var("ALLOWED_PUBKEYS");env::remove_var("KEYCAST_OPERATOR_PUBKEYS");
}

#[test]
fn external_approval_has_a_bounded_human_review_window() {
    let req=request(Method::POST,"/teams");
    let now=chrono::Utc::now().timestamp();
    for (age,accepted) in [(70,true),(240,true),(301,false)] {
        let event=auth_event(standard_tags("POST","http://example.com/api/teams"),now-age);
        assert_eq!(validate_auth_event(&event,&req,&[],"http://example.com/api").is_ok(),accepted);
    }
}

#[tokio::test]
async fn discovery_policy_requires_operator_and_signed_approval() {
    let _guard=ENV_LOCK.lock().unwrap(); let pool=setup_route_test_db().await;
    let operator=Keys::generate(); let member=Keys::generate();
    env::set_var("ALLOWED_PUBKEYS",format!("{},{}",operator.public_key().to_hex(),member.public_key().to_hex()));
    env::set_var("KEYCAST_OPERATOR_PUBKEYS",operator.public_key().to_hex());
    let app=routes::routes(state(pool.clone()));
    assert_eq!(route_request(&app,&pool,&member,"PUT","/relay-discovery",r#"{"auto_activate":true}"#).await.status,403);
    assert!(!query_scalar::<_,bool>("SELECT auto_activate FROM relay_discovery_policy").fetch_one(&pool).await.unwrap());
    assert_eq!(route_request(&app,&pool,&operator,"PUT","/relay-discovery",r#"{"auto_activate":true}"#).await.status,204);
    assert!(query_scalar::<_,bool>("SELECT auto_activate FROM relay_discovery_policy").fetch_one(&pool).await.unwrap());
    assert_eq!(route_request(&app,&pool,&operator,"PUT","/relay-discovery",r#"{"auto_activate":true,"allow_private":true}"#).await.status,422);
    env::remove_var("ALLOWED_PUBKEYS"); env::remove_var("KEYCAST_OPERATOR_PUBKEYS");
}

#[tokio::test]
async fn team_delete_removes_grants_that_would_otherwise_block_the_cascade() {
    let _guard = ENV_LOCK.lock().unwrap();
    let pool = setup_route_test_db().await;
    let admin = Keys::generate();
    env::set_var("ALLOWED_PUBKEYS", admin.public_key().to_hex());
    let app = crate::management::api::http::routes::routes(state(pool.clone()));

    let team: i64 = query_scalar("INSERT INTO teams(name) VALUES('Doomed') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    member_row(&pool, team, &admin, "admin").await;
    let stored_key: i64 = query_scalar(
        "INSERT INTO stored_keys(team_id,name,public_key,secret_envelope,envelope_version,key_encryption_key_id) VALUES(?,'k',?,x'01',1,'kid') RETURNING id")
        .bind(team)
        .bind(Keys::generate().public_key().to_hex())
        .fetch_one(&pool)
        .await
        .unwrap();
    let policy: i64 = query_scalar(
        "INSERT INTO policies(team_id,name,document) VALUES(?,'p','{\"version\":1}') RETURNING id",
    )
    .bind(team)
    .fetch_one(&pool)
    .await
    .unwrap();
    // A revoked grant is a tombstone, so it blocks the cascade exactly like a live one.
    for (name, revoked) in [("live", false), ("revoked", true)] {
        let grant: i64 = query_scalar(
            "INSERT INTO grants(team_id,stored_key_id,policy_id,name,remote_signer_public_key,remote_signer_secret_envelope,envelope_version,key_encryption_key_id) VALUES(?,?,?,?,?,x'01',1,'kid') RETURNING id")
            .bind(team).bind(stored_key).bind(policy).bind(name)
            .bind(Keys::generate().public_key().to_hex())
            .fetch_one(&pool).await.unwrap();
        let invitation: i64 = query_scalar(
            "INSERT INTO invitations(grant_id,secret_hash,expires_at) VALUES(?,randomblob(32),unixepoch()+3600) RETURNING id")
            .bind(grant).fetch_one(&pool).await.unwrap();
        query("INSERT INTO sessions(grant_id,invitation_id,client_public_key) VALUES(?,?,?)")
            .bind(grant)
            .bind(invitation)
            .bind(Keys::generate().public_key().to_hex())
            .execute(&pool)
            .await
            .unwrap();
        if revoked {
            query("UPDATE grants SET revoked_at=unixepoch() WHERE id=?")
                .bind(grant)
                .execute(&pool)
                .await
                .unwrap();
        }
    }

    let path = format!("/teams/{team}");
    let reply = route_request(&app, &pool, &admin, "DELETE", &path, "").await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT.as_u16());
    let remaining: (i64, i64, i64, i64, i64, i64, i64) = query_as(
        "SELECT (SELECT count(*) FROM teams),(SELECT count(*) FROM grants),
                (SELECT count(*) FROM sessions),(SELECT count(*) FROM invitations),
                (SELECT count(*) FROM stored_keys),(SELECT count(*) FROM policies),
                (SELECT count(*) FROM team_members)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, (0, 0, 0, 0, 0, 0, 0));
    // The audit trail outlives the team, with its links cleared.
    let recorded: i64 =
        query_scalar("SELECT count(*) FROM audit_events WHERE action='team.delete'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(recorded, 1);
    env::remove_var("ALLOWED_PUBKEYS");
}

#[tokio::test]
async fn approval_content_carrying_private_material_is_refused() {
    let _guard = ENV_LOCK.lock().unwrap();
    let pool = setup_route_test_db().await;
    let admin = Keys::generate();
    env::set_var("ALLOWED_PUBKEYS", admin.public_key().to_hex());
    let app = crate::management::api::http::routes::routes(state(pool.clone()));
    let team: i64 = query_scalar("INSERT INTO teams(name) VALUES('Keys') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    member_row(&pool, team, &admin, "admin").await;
    let endpoint = format!("/teams/{team}/keys");
    let imported = Keys::generate();

    // An operator pasting a private key into the name field would otherwise ship it
    // to the external signer inside the approval event's content.
    let pasted = format!(
        r#"{{"name":"{}","secret_key":"{}"}}"#,
        imported.secret_key().to_bech32().unwrap(),
        imported.secret_key().to_secret_hex()
    );
    let reply = route_request(&app, &pool, &admin, "POST", &endpoint, &pasted).await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST.as_u16());
    assert!(!reply.body.contains("nsec1"));

    // A named import still works, and its approval never carries the key.
    let clean = format!(
        r#"{{"name":"Personal identity","secret_key":"{}"}}"#,
        imported.secret_key().to_secret_hex()
    );
    let reply = route_request(&app, &pool, &admin, "POST", &endpoint, &clean).await;
    assert_eq!(reply.status, StatusCode::CREATED.as_u16());
    let stored: i64 = query_scalar("SELECT count(*) FROM stored_keys")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, 1);
    env::remove_var("ALLOWED_PUBKEYS");
}

#[tokio::test]
async fn management_replies_come_from_the_published_stable_identity() {
    let _guard = ENV_LOCK.lock().unwrap();
    let pool = setup_route_test_db().await;
    let admin = Keys::generate();
    env::set_var("ALLOWED_PUBKEYS", admin.public_key().to_hex());
    let app = crate::management::api::http::routes::routes(state(pool.clone()));

    // The browser learns the reply identity from /config and pins it.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/config?pubkey={}", admin.public_key().to_hex()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let config: serde_json::Value = serde_json::from_slice(
        &to_bytes(response.into_body(), MAX_AUTH_BODY_BYTES)
            .await
            .unwrap(),
    )
    .unwrap();
    let published = config["management_reply_public_key"]
        .as_str()
        .expect("published reply identity")
        .to_owned();
    let pinned = PublicKey::from_hex(&published).unwrap();

    // Every write reply must come from that identity, not a per-reply ephemeral key.
    let mut senders = BTreeSet::new();
    for name in ["First", "Second"] {
        let body = format!(r#"{{"name":"{name}"}}"#);
        let revision: i64 = query_scalar("SELECT authority_revision FROM instance_settings")
            .fetch_one(&pool)
            .await
            .unwrap();
        let response = app
            .clone()
            .oneshot(signed_request(&admin, "POST", "/teams", &body, revision))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let envelope: keycast_core::v2::control::EncryptedReply = serde_json::from_slice(
            &to_bytes(response.into_body(), MAX_AUTH_BODY_BYTES)
                .await
                .unwrap(),
        )
        .unwrap();
        senders.insert(envelope.public_key.clone());
        let plain = nostr::nips::nip44::decrypt(
            admin.secret_key(),
            &pinned,
            &envelope.encrypted_response,
        )
        .expect("reply authenticates under the pinned identity");
        let reply: keycast_core::v2::control::HttpReply = serde_json::from_str(&plain).unwrap();
        assert_eq!(reply.status, StatusCode::CREATED.as_u16());
    }
    assert_eq!(senders, BTreeSet::from([published]));

    // A reply forged by anyone else, including the API, fails to authenticate.
    let forged = nostr::nips::nip44::encrypt(
        Keys::generate().secret_key(),
        &admin.public_key(),
        r#"{"status":201,"body":"forged"}"#,
        nostr::nips::nip44::Version::default(),
    )
    .unwrap();
    assert!(nostr::nips::nip44::decrypt(admin.secret_key(), &pinned, &forged).is_err());
    env::remove_var("ALLOWED_PUBKEYS");
}
