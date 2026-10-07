#![cfg(feature = "turso")]

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
