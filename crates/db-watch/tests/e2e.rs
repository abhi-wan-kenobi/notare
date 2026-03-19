use db_watch::{TableDeps, extract_tables};
use sqlx::SqlitePool;

async fn test_pool() -> SqlitePool {
    let db = hypr_db_core2::Db3::connect_memory_plain().await.unwrap();
    hypr_db_app::migrate(db.pool()).await.unwrap();
    db.pool().clone()
}

#[tokio::test]
async fn full_cycle() {
    let pool = test_pool().await;
    let tables = extract_tables(&pool, "SELECT id FROM sessions WHERE id = ?")
        .await
        .unwrap();

    let mut deps = TableDeps::new();
    let w = deps.register(tables);

    assert!(deps.affected(&["sessions"]).contains(&w));
    assert!(!deps.affected(&["words"]).contains(&w));
}

#[tokio::test]
async fn multi_table_join() {
    let pool = test_pool().await;
    let tables = extract_tables(
        &pool,
        "SELECT w.id FROM words w JOIN sessions s ON w.session_id = s.id",
    )
    .await
    .unwrap();

    let mut deps = TableDeps::new();
    let w = deps.register(tables);

    assert!(deps.affected(&["sessions"]).contains(&w));
    assert!(deps.affected(&["words"]).contains(&w));
    assert!(!deps.affected(&["chat_messages"]).contains(&w));
}

#[tokio::test]
async fn unregister_stops_notifications() {
    let pool = test_pool().await;
    let tables = extract_tables(&pool, "SELECT id FROM sessions WHERE id = ?")
        .await
        .unwrap();

    let mut deps = TableDeps::new();
    let w = deps.register(tables);

    assert!(deps.affected(&["sessions"]).contains(&w));

    deps.unregister(w);
    assert!(!deps.affected(&["sessions"]).contains(&w));
}

#[tokio::test]
async fn overlapping_watches() {
    let pool = test_pool().await;

    let tables_a = extract_tables(
        &pool,
        "SELECT w.id FROM words w JOIN sessions s ON w.session_id = s.id",
    )
    .await
    .unwrap();

    let tables_b = extract_tables(
        &pool,
        "SELECT c.id FROM chat_messages c JOIN sessions s ON c.session_id = s.id",
    )
    .await
    .unwrap();

    let mut deps = TableDeps::new();
    let a = deps.register(tables_a);
    let b = deps.register(tables_b);

    let words_hit = deps.affected(&["words"]);
    assert!(words_hit.contains(&a));
    assert!(!words_hit.contains(&b));

    let chat_hit = deps.affected(&["chat_messages"]);
    assert!(!chat_hit.contains(&a));
    assert!(chat_hit.contains(&b));

    let sessions_hit = deps.affected(&["sessions"]);
    assert!(sessions_hit.contains(&a));
    assert!(sessions_hit.contains(&b));
}

#[tokio::test]
async fn fts_watch_cycle() {
    let pool = test_pool().await;
    let tables = extract_tables(
        &pool,
        "SELECT rowid FROM sessions_fts WHERE sessions_fts MATCH 'test'",
    )
    .await
    .unwrap();

    assert!(!tables.is_empty());

    let mut deps = TableDeps::new();
    let w = deps.register(tables);

    assert!(deps.affected(&["sessions_fts"]).contains(&w));
    assert!(!deps.affected(&["sessions"]).contains(&w));
}
