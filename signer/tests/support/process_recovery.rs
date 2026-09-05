// Included by hardening.rs: these use the real daemon binary, private socket and file-backed WAL.
struct Daemon(std::process::Child);
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn daemon(directory: &std::path::Path) -> Daemon {
    Daemon(
        std::process::Command::new(env!("CARGO_BIN_EXE_keycast_signer"))
            .env("KEYCAST_DATABASE_PATH", directory.join("source.db"))
            .env("KEYCAST_ROOT_KEY_FILE", directory.join("root.key"))
            .env("KEYCAST_SIGNER_SOCKET", directory.join("signer.sock"))
            .env("KEYCAST_PUBLIC_URL", "http://127.0.0.1/api")
            .env(
                "KEYCAST_MIGRATIONS_PATH",
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("database/migrations"),
            )
            .env_remove("CREDENTIALS_DIRECTORY")
            .env("RUST_LOG", "error")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}
async fn daemon_status(
    directory: &std::path::Path,
) -> Option<keycast_core::v2::control::SignerStatus> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut socket = tokio::net::UnixStream::connect(directory.join("signer.sock"))
            .await
            .ok()?;
        socket.write_all(br#"{"operation":"status"}"#).await.ok()?;
        socket.shutdown().await.ok()?;
        let mut bytes = Vec::new();
        socket.take(65536).read_to_end(&mut bytes).await.ok()?;
        match serde_json::from_slice(&bytes).ok()? {
            keycast_core::v2::control::ControlResponse::Status { status } => Some(status),
            _ => None,
        }
    })
    .await
    .ok()
    .flatten()
}
async fn daemon_ready(directory: &std::path::Path, daemon: &mut Daemon) {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            assert!(
                daemon.0.try_wait().unwrap().is_none(),
                "daemon exited before readiness"
            );
            if daemon_status(directory).await.is_some_and(|s| s.ready) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("daemon readiness");
}
async fn file_fixture() -> (std::path::PathBuf, Fixture) {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let directory = std::path::Path::new("/tmp").join(format!(
        "keycast-process-test-{}",
        &Keys::generate().public_key().to_hex()[..16]
    ));
    std::fs::create_dir(&directory).unwrap();
    let mut key = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory.join("root.key"))
        .unwrap();
    key.write_all("15".repeat(32).as_bytes()).unwrap();
    let db = keycast_core::database::Database::new(
        directory.join("source.db"),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("database/migrations"),
    )
    .await
    .unwrap();
    let f = Fixture::populate(db.pool, "ws://127.0.0.1:1").await;
    (directory, f)
}
#[derive(Debug)]
struct RejectReplies {
    remote: PublicKey,
    accept: Arc<std::sync::atomic::AtomicBool>,
}
impl WritePolicy for RejectReplies {
    fn admit_event<'a>(
        &'a self,
        event: &'a Event,
        _: &'a std::net::SocketAddr,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = WritePolicyResult> + Send + 'a>> {
        Box::pin(async move {
            if event.pubkey == self.remote && !self.accept.load(Ordering::SeqCst) {
                WritePolicyResult::reject(MachineReadablePrefix::Blocked, "test outage")
            } else {
                WritePolicyResult::Accept
            }
        })
    }
}
#[tokio::test]
async fn sigkill_after_commit_recovers_response_and_keeps_session_with_one_authority() {
    let (directory, f) = file_fixture().await;
    let accept = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let relay = LocalRelay::builder()
        .write_policy(RejectReplies {
            remote: f.remote.public_key(),
            accept: accept.clone(),
        })
        .build();
    relay.run().await.unwrap();
    query("UPDATE relays SET url=?")
        .bind(relay.url().await.as_str())
        .execute(&f.store.pool)
        .await
        .unwrap();
    let observer = Client::new();
    observer.add_relay(relay.url().await).await.unwrap();
    let mut stream = observer.notifications();
    observer.connect().and_wait(Duration::from_secs(2)).await;
    observer
        .subscribe(
            Filter::new()
                .kind(Kind::NostrConnect)
                .pubkey(f.client.public_key()),
        )
        .await
        .unwrap();
    let mut child = daemon(&directory);
    daemon_ready(&directory, &mut child).await;
    // A second OS process must fail without unlinking the active control socket.
    let mut second = daemon(&directory);
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(status) = second.0.try_wait().unwrap() {
                assert!(!status.success());
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(daemon_status(&directory).await.unwrap().ready);
    let connect = f.connect();
    observer.send_event(&connect).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5),async {
        loop {
            let committed:bool=query_scalar("SELECT EXISTS(SELECT 1 FROM processed_requests WHERE event_id=? AND response_event_json IS NOT NULL)").bind(connect.id.to_hex()).fetch_one(&f.store.pool).await.unwrap();
            if committed {break;} tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    child.0.kill().unwrap();
    child.0.wait().unwrap(); // SIGKILL: no daemon shutdown or flush path.
    accept.store(true, Ordering::SeqCst);
    let mut restarted = daemon(&directory);
    daemon_ready(&directory, &mut restarted).await;
    assert_eq!(reply(&mut stream, "connect", &f).await["result"], "ack");
    assert_eq!(
        query_scalar::<_, i64>("SELECT count(*) FROM sessions WHERE ended_at IS NULL")
            .fetch_one(&f.store.pool)
            .await
            .unwrap(),
        1
    );
    observer
        .send_event(&f.event("after-kill", "ping", vec![]))
        .await
        .unwrap();
    assert_eq!(reply(&mut stream, "after-kill", &f).await["result"], "pong");
    drop(restarted);
    drop(child);
    drop(second);
    observer.shutdown().await;
    relay.shutdown();
    f.store.pool.close().await;
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn sqlite_full_rolls_back_admission_and_recovers_after_space_is_available() {
    let f = Fixture::new("ws://127.0.0.1:1").await;
    f.processor().prepare_response(&f.connect()).await.unwrap();
    // SQLite's documented page ceiling produces SQLITE_FULL without filling the host disk.
    let pages: i64 = query_scalar("PRAGMA page_count")
        .fetch_one(&f.store.pool)
        .await
        .unwrap();
    query(sqlx::sql_str::AssertSqlSafe(format!(
        "PRAGMA max_page_count={pages}"
    )))
    .execute(&f.store.pool)
    .await
    .unwrap();
    let oversized = f.event(
        "full",
        "sign_event",
        vec![
            json!({"created_at":Timestamp::now(),"kind":1,"tags":[],"content":"x".repeat(32000)})
                .to_string(),
        ],
    );
    let result = f.processor().prepare_response(&oversized).await;
    assert!(
        matches!(
            result,
            Err(keycast_signer::protocol::ProtocolError::Store(_))
        ),
        "SQLITE_FULL must fail closed"
    );
    assert!(!query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM processed_requests WHERE event_id=?)"
    )
    .bind(oversized.id.to_hex())
    .fetch_one(&f.store.pool)
    .await
    .unwrap());
    query("PRAGMA max_page_count=65536")
        .execute(&f.store.pool)
        .await
        .unwrap();
    let response = f
        .processor()
        .prepare_response(&oversized)
        .await
        .unwrap()
        .unwrap();
    assert!(f.decrypt(&response).get("result").is_some());
    assert_eq!(
        query_scalar::<_, i64>("SELECT records FROM request_storage_budget")
            .fetch_one(&f.store.pool)
            .await
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn busy_database_does_not_sign_or_drop_retry_and_recovers_when_lock_releases() {
    let (directory, f) = file_fixture().await;
    f.processor().prepare_response(&f.connect()).await.unwrap();
    let locked = f.store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let request = f.event("busy", "ping", vec![]);
    let event_id = request.id.to_hex();
    let processor = f.processor();
    let mut worker = tokio::spawn(async move { processor.prepare_response(&request).await });
    assert!(
        tokio::time::timeout(Duration::from_millis(150), &mut worker)
            .await
            .is_err()
    );
    assert!(!query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM processed_requests WHERE event_id=?)"
    )
    .bind(event_id)
    .fetch_one(&f.store.pool)
    .await
    .unwrap());
    locked.rollback().await.unwrap();
    let response = tokio::time::timeout(Duration::from_secs(3), worker)
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(f.decrypt(&response)["result"], "pong");
    f.store.pool.close().await;
    std::fs::remove_dir_all(directory).unwrap();
}
