use db_watch::{TableDeps, extract_tables};

async fn test_db() -> hypr_db_core2::Db3 {
    let db = hypr_db_core2::Db3::connect_memory_plain().await.unwrap();
    hypr_db_app::migrate(db.pool()).await.unwrap();
    db
}

#[tokio::test]
async fn full_cycle() {
    let db = test_db().await;
    let tables = extract_tables(db.pool(), "SELECT id FROM daily_notes WHERE id = ?")
        .await
        .unwrap();

    let mut deps = TableDeps::new();
    let w = deps.register(tables);

    assert!(deps.affected(&["daily_notes"]).contains(&w));
    assert!(!deps.affected(&["daily_summaries"]).contains(&w));
}

#[tokio::test]
async fn multi_table_join() {
    let db = test_db().await;
    let tables = extract_tables(
        db.pool(),
        "SELECT ds.id FROM daily_summaries ds JOIN daily_notes dn ON ds.daily_note_id = dn.id",
    )
    .await
    .unwrap();

    let mut deps = TableDeps::new();
    let w = deps.register(tables);

    assert!(deps.affected(&["daily_notes"]).contains(&w));
    assert!(deps.affected(&["daily_summaries"]).contains(&w));
    assert!(!deps.affected(&["missing"]).contains(&w));
}

#[tokio::test]
async fn unregister_stops_notifications() {
    let db = test_db().await;
    let tables = extract_tables(db.pool(), "SELECT id FROM daily_notes WHERE id = ?")
        .await
        .unwrap();

    let mut deps = TableDeps::new();
    let w = deps.register(tables);

    assert!(deps.affected(&["daily_notes"]).contains(&w));

    deps.unregister(w);
    assert!(!deps.affected(&["daily_notes"]).contains(&w));
}

#[tokio::test]
async fn overlapping_watches() {
    let db = test_db().await;

    let tables_a = extract_tables(
        db.pool(),
        "SELECT ds.id FROM daily_summaries ds JOIN daily_notes dn ON ds.daily_note_id = dn.id",
    )
    .await
    .unwrap();

    let tables_b = extract_tables(
        db.pool(),
        "SELECT id FROM daily_notes \
         WHERE EXISTS ( \
           SELECT 1 FROM daily_summaries \
           WHERE daily_summaries.daily_note_id = daily_notes.id \
         )",
    )
    .await
    .unwrap();

    let mut deps = TableDeps::new();
    let a = deps.register(tables_a);
    let b = deps.register(tables_b);

    let summaries_hit = deps.affected(&["daily_summaries"]);
    assert!(summaries_hit.contains(&a));
    assert!(summaries_hit.contains(&b));

    let notes_hit = deps.affected(&["daily_notes"]);
    assert!(notes_hit.contains(&a));
    assert!(notes_hit.contains(&b));
}

#[tokio::test]
async fn alias_watch_cycle() {
    let db = test_db().await;
    let tables = extract_tables(
        db.pool(),
        "SELECT dn.id FROM daily_notes AS dn WHERE dn.date = '2026-04-11'",
    )
    .await
    .unwrap();

    assert!(!tables.is_empty());

    let mut deps = TableDeps::new();
    let w = deps.register(tables);

    assert!(deps.affected(&["daily_notes"]).contains(&w));
    assert!(!deps.affected(&["daily_summaries"]).contains(&w));
}
