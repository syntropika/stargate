#![cfg(feature = "turso")]

use storage::{AuthStore, ExternalIdentity, User, UserRole, adapters::turso::TursoStore};

#[tokio::test]
async fn initial_schema_supports_mvcc() {
    let db = turso::Builder::new_local(":memory:").build().await.unwrap();
    let conn = db.connect().unwrap();
    conn.pragma_update("journal_mode", "'mvcc'").await.unwrap();
    conn.execute("PRAGMA foreign_keys = ON", ()).await.unwrap();
    conn.execute("BEGIN EXCLUSIVE", ()).await.unwrap();
    conn.execute_batch(include_str!("../src/adapters/turso/migrations/0001.sql"))
        .await
        .unwrap();
    conn.execute("COMMIT", ()).await.unwrap();
}

#[tokio::test]
async fn schema_one_preserves_users_sessions_and_explicit_admin_initialization() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.db");
    {
        let db = turso::Builder::new_local(path.to_str().unwrap())
            .build()
            .await
            .unwrap();
        let conn = db.connect().unwrap();
        conn.pragma_update("journal_mode", "'mvcc'").await.unwrap();
        conn.execute_batch(include_str!("../src/adapters/turso/migrations/0001.sql"))
            .await
            .unwrap();
        conn.execute_batch("CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY); INSERT INTO schema_migrations VALUES(1);
            INSERT INTO users VALUES('existing','same@example.com',1,'{\"id\":\"existing\",\"email\":\"same@example.com\",\"created_at\":1}');
            INSERT INTO sessions VALUES('session','existing','hash',9999,NULL,NULL,'{\"id\":\"session\",\"user_id\":\"existing\",\"token_hash\":\"hash\",\"created_at\":1,\"expires_at\":9999,\"revoked_at\":null,\"last_used_at\":null}');").await.unwrap();
    }
    let store = TursoStore::open(path.to_str().unwrap(), 16, 32, None)
        .await
        .unwrap();
    assert_eq!(
        store.get_user("existing").await.unwrap().unwrap().role,
        UserRole::User
    );
    assert!(store.find_session("hash", 2).await.unwrap().is_some());
    assert!(!store.local_setup_available().await.unwrap());
    assert!(store.prepare_local_accounts(None).await.is_err());
    assert!(store.prepare_local_accounts(Some("missing")).await.is_err());
    store
        .prepare_local_accounts(Some("existing"))
        .await
        .unwrap();
    assert_eq!(
        store.get_user("existing").await.unwrap().unwrap().role,
        UserRole::Administrator
    );
    let external = User {
        id: "second".into(),
        email: Some("same@example.com".into()),
        created_at: 2,
        role: UserRole::User,
        disabled_at: None,
    };
    let external = store
        .resolve_identity(
            &ExternalIdentity {
                issuer: "https://identity.example.com".into(),
                subject: "second".into(),
                email: external.email.clone(),
                metadata: serde_json::json!({}),
            },
            &external,
        )
        .await
        .unwrap();
    assert_ne!(external.id, "existing");
    store
        .update_user("existing", &external.id, UserRole::Administrator, None)
        .await
        .unwrap()
        .unwrap();
    store
        .update_user("existing", "existing", UserRole::User, None)
        .await
        .unwrap()
        .unwrap();
    drop(store);
    let store = TursoStore::open(path.to_str().unwrap(), 16, 32, None)
        .await
        .unwrap();
    store
        .prepare_local_accounts(Some("existing"))
        .await
        .unwrap();
    assert_eq!(
        store.get_user("existing").await.unwrap().unwrap().role,
        UserRole::User
    );
    assert!(store.find_session("hash", 3).await.unwrap().is_some());
}
