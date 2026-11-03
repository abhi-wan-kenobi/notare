use db_migrate::{DbSchema, MigrateError, MigrationScope, MigrationStep, migrate};
use hypr_db_core::Db;

const CREATE_WIDGETS_SQL: &str = r#"
CREATE TABLE widgets (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL DEFAULT ''
)
"#;

const CREATE_SYNCED_WIDGETS_SQL: &str = r#"
CREATE TABLE widgets (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL DEFAULT ''
)
"#;

const CREATE_WIDGETS_WITH_STATUS_SQL: &str = r#"
CREATE TABLE widgets (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'draft'
)
"#;

const SEED_WIDGETS_SQL: &str = r#"
INSERT INTO widgets (id, name) VALUES (1, 'alpha')
"#;

const ADD_SLUG_SQL: &str = r#"
ALTER TABLE widgets ADD COLUMN slug TEXT NOT NULL DEFAULT ''
"#;

const PLAIN_SCHEMA_STEPS: &[MigrationStep] = &[
    MigrationStep {
        id: "20260415010101_create_widgets",
        scope: MigrationScope::Plain,
        sql: CREATE_WIDGETS_SQL,
    },
    MigrationStep {
        id: "20260415010102_seed_widgets",
        scope: MigrationScope::Plain,
        sql: SEED_WIDGETS_SQL,
    },
];

const MODIFIED_PLAIN_SCHEMA_STEPS: &[MigrationStep] = &[MigrationStep {
    id: "20260415010101_create_widgets",
    scope: MigrationScope::Plain,
    sql: CREATE_WIDGETS_WITH_STATUS_SQL,
}];

const MISSING_VERSION_SCHEMA_STEPS: &[MigrationStep] = &[MigrationStep {
    id: "20260415010102_seed_widgets",
    scope: MigrationScope::Plain,
    sql: SEED_WIDGETS_SQL,
}];

const DUPLICATE_VERSION_SCHEMA_STEPS: &[MigrationStep] = &[
    MigrationStep {
        id: "20260415020101_create_widgets",
        scope: MigrationScope::Plain,
        sql: CREATE_WIDGETS_SQL,
    },
    MigrationStep {
        id: "20260415020101_seed_widgets",
        scope: MigrationScope::Plain,
        sql: SEED_WIDGETS_SQL,
    },
];

const INVALID_STEP_ID_SCHEMA_STEPS: &[MigrationStep] = &[MigrationStep {
    id: "invalid_step_id",
    scope: MigrationScope::Plain,
    sql: CREATE_WIDGETS_SQL,
}];

const SYNCED_BASE_STEPS: &[MigrationStep] = &[MigrationStep {
    id: "20260415030101_create_widgets",
    scope: MigrationScope::Plain,
    sql: CREATE_SYNCED_WIDGETS_SQL,
}];

const CRR_ALTER_STEPS: &[MigrationStep] = &[
    MigrationStep {
        id: "20260415030101_create_widgets",
        scope: MigrationScope::Plain,
        sql: CREATE_SYNCED_WIDGETS_SQL,
    },
    MigrationStep {
        id: "20260415030102_add_slug",
        scope: MigrationScope::CrrAlter {
            table_name: "widgets",
        },
        sql: ADD_SLUG_SQL,
    },
];

fn schema(steps: &'static [MigrationStep], validate_crr_table: fn(&str) -> bool) -> DbSchema {
    DbSchema {
        steps,
        validate_crr_table,
    }
}

fn never_synced(_: &str) -> bool {
    false
}

fn widgets_synced(table_name: &str) -> bool {
    table_name == "widgets"
}

async fn open_plain_db() -> Db {
    Db::connect_memory_plain().await.unwrap()
}

async fn applied_versions(db: &Db) -> Vec<i64> {
    sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
        .fetch_all(db.pool())
        .await
        .unwrap()
}

async fn widget_names(db: &Db) -> Vec<String> {
    sqlx::query_scalar("SELECT name FROM widgets ORDER BY id")
        .fetch_all(db.pool())
        .await
        .unwrap()
}

async fn widget_columns(db: &Db) -> Vec<String> {
    sqlx::query_scalar("SELECT name FROM pragma_table_info('widgets') ORDER BY cid")
        .fetch_all(db.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn plain_migrations_apply_and_remain_idempotent() {
    let db = open_plain_db().await;

    migrate(&db, schema(PLAIN_SCHEMA_STEPS, never_synced))
        .await
        .unwrap();

    assert_eq!(
        applied_versions(&db).await,
        vec![20260415010101, 20260415010102]
    );
    assert_eq!(widget_names(&db).await, vec!["alpha".to_string()]);

    migrate(&db, schema(PLAIN_SCHEMA_STEPS, never_synced))
        .await
        .unwrap();

    assert_eq!(
        applied_versions(&db).await,
        vec![20260415010101, 20260415010102]
    );
    assert_eq!(widget_names(&db).await, vec!["alpha".to_string()]);
}

#[tokio::test]
async fn changed_checksum_is_rejected() {
    let db = open_plain_db().await;

    migrate(&db, schema(&PLAIN_SCHEMA_STEPS[..1], never_synced))
        .await
        .unwrap();

    let err = migrate(&db, schema(MODIFIED_PLAIN_SCHEMA_STEPS, never_synced))
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        MigrateError::SqlxMigrate(sqlx::migrate::MigrateError::VersionMismatch(20260415010101))
    ));
}

#[tokio::test]
async fn missing_applied_version_is_rejected() {
    let db = open_plain_db().await;

    migrate(&db, schema(PLAIN_SCHEMA_STEPS, never_synced))
        .await
        .unwrap();

    let err = migrate(&db, schema(MISSING_VERSION_SCHEMA_STEPS, never_synced))
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        MigrateError::SqlxMigrate(sqlx::migrate::MigrateError::VersionMissing(20260415010101))
    ));
}

#[tokio::test]
async fn invalid_manifest_metadata_is_rejected() {
    let db = open_plain_db().await;

    let invalid_id = migrate(&db, schema(INVALID_STEP_ID_SCHEMA_STEPS, never_synced))
        .await
        .unwrap_err();
    assert!(matches!(
        invalid_id,
        MigrateError::InvalidStepId {
            step_id: "invalid_step_id"
        }
    ));

    let duplicate_version = migrate(&db, schema(DUPLICATE_VERSION_SCHEMA_STEPS, never_synced))
        .await
        .unwrap_err();
    assert!(matches!(
        duplicate_version,
        MigrateError::DuplicateStepVersion {
            version: 20260415020101,
            ..
        }
    ));
}

#[tokio::test]
async fn crr_alter_scope_runs_plain_without_crsqlite() {
    let db = open_plain_db().await;

    migrate(&db, schema(SYNCED_BASE_STEPS, widgets_synced))
        .await
        .unwrap();
    migrate(&db, schema(CRR_ALTER_STEPS, widgets_synced))
        .await
        .unwrap();

    assert_eq!(
        applied_versions(&db).await,
        vec![20260415030101, 20260415030102]
    );
    assert_eq!(
        widget_columns(&db).await,
        vec!["id".to_string(), "name".to_string(), "slug".to_string()]
    );
}

#[tokio::test]
async fn crr_alter_step_on_an_unsynced_table_is_rejected() {
    let db = open_plain_db().await;

    let error = migrate(&db, schema(CRR_ALTER_STEPS, never_synced))
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        MigrateError::InvalidCrrStep {
            step_id: "20260415030102_add_slug",
            table_name: "widgets",
        }
    ));
    assert!(widget_columns(&db).await.is_empty());
}

#[cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
mod crsqlite {
    use super::*;
    use hypr_db_core::{DbOpenOptions, DbStorage};

    const ADD_SLUG_WITHOUT_DEFAULT_SQL: &str = r#"
ALTER TABLE widgets ADD COLUMN slug TEXT NOT NULL
"#;

    const FAILING_CRR_ALTER_STEPS: &[MigrationStep] = &[
        MigrationStep {
            id: "20260415030101_create_widgets",
            scope: MigrationScope::Plain,
            sql: CREATE_SYNCED_WIDGETS_SQL,
        },
        MigrationStep {
            id: "20260415030102_add_slug",
            scope: MigrationScope::CrrAlter {
                table_name: "widgets",
            },
            sql: ADD_SLUG_WITHOUT_DEFAULT_SQL,
        },
    ];

    // File-backed: an alter window closes connections, and a fresh
    // connection to `sqlite::memory:` would be a different, empty database.
    async fn open_crsqlite_db() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_crsqlite(DbOpenOptions {
            storage: DbStorage::Local(&dir.path().join("app.db")),
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(2),
        })
        .await
        .unwrap();
        (dir, db)
    }

    async fn changed_columns(db: &Db) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT DISTINCT cid FROM crsql_changes WHERE \"table\" = 'widgets' ORDER BY cid",
        )
        .fetch_all(db.pool())
        .await
        .unwrap()
    }

    async fn insert_widget(db: &Db, id: &str) {
        sqlx::query("INSERT INTO widgets (id, name) VALUES (?, 'widget')")
            .bind(id)
            .execute(db.pool())
            .await
            .unwrap();
    }

    async fn open_synced_widgets() -> (tempfile::TempDir, Db) {
        let (dir, db) = open_crsqlite_db().await;
        migrate(&db, schema(SYNCED_BASE_STEPS, widgets_synced))
            .await
            .unwrap();
        hypr_crsqlite::as_crr(db.pool(), "widgets").await.unwrap();
        (dir, db)
    }

    /// After a failed `CrrAlter` step: nothing of it is applied, the table
    /// is still the same CRR (new writes track only the real columns), and
    /// the corrected step then applies.
    async fn assert_failed_alter_left_the_crr_intact(db: &Db, error: MigrateError) {
        assert!(matches!(error, MigrateError::SqlxMigrate(_)), "{error:?}");
        assert_eq!(applied_versions(db).await, vec![20260415030101]);
        assert_eq!(
            widget_columns(db).await,
            vec!["id".to_string(), "name".to_string()]
        );
        insert_widget(db, "w1").await;
        assert_eq!(changed_columns(db).await, vec!["name"]);

        migrate(db, schema(CRR_ALTER_STEPS, widgets_synced))
            .await
            .unwrap();
        assert_eq!(
            applied_versions(db).await,
            vec![20260415030101, 20260415030102]
        );
        sqlx::query("UPDATE widgets SET slug = 'w-1' WHERE id = 'w1'")
            .execute(db.pool())
            .await
            .unwrap();
        assert_eq!(changed_columns(db).await, vec!["name", "slug"]);
    }

    #[tokio::test]
    async fn crr_alter_scope_keeps_new_columns_tracked_on_a_crr() {
        let (_dir, db) = open_synced_widgets().await;
        // Writes on both pooled connections before the alter, so each one's
        // cr-sqlite state describes the old schema when the window opens.
        {
            let mut a = db.pool().acquire().await.unwrap();
            let mut b = db.pool().acquire().await.unwrap();
            for (conn, id) in [(&mut a, "w0"), (&mut b, "w1")] {
                sqlx::query("INSERT INTO widgets (id, name) VALUES (?, 'widget')")
                    .bind(id)
                    .execute(&mut **conn)
                    .await
                    .unwrap();
            }
        }

        migrate(&db, schema(CRR_ALTER_STEPS, widgets_synced))
            .await
            .unwrap();

        let mut a = db.pool().acquire().await.unwrap();
        let mut b = db.pool().acquire().await.unwrap();
        for (conn, id) in [(&mut a, "w0"), (&mut b, "w1")] {
            sqlx::query("UPDATE widgets SET slug = ? WHERE id = ?")
                .bind(format!("{id}-slug"))
                .bind(id)
                .execute(&mut **conn)
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO widgets (id, name, slug) VALUES ('w2', 'widget', 'w-2')")
            .execute(&mut *a)
            .await
            .unwrap();
        drop((a, b));

        assert_eq!(
            applied_versions(&db).await,
            vec![20260415030101, 20260415030102]
        );
        assert_eq!(changed_columns(&db).await, vec!["name", "slug"]);
        db.close().await;
    }

    #[tokio::test]
    async fn crr_alter_scope_does_not_convert_a_table_that_is_not_a_crr() {
        let (_dir, db) = open_crsqlite_db().await;

        migrate(&db, schema(CRR_ALTER_STEPS, widgets_synced))
            .await
            .unwrap();

        assert_eq!(
            widget_columns(&db).await,
            vec!["id".to_string(), "name".to_string(), "slug".to_string()]
        );
        assert!(!hypr_crsqlite::is_crr(db.pool(), "widgets").await.unwrap());
        db.close().await;
    }

    #[tokio::test]
    async fn crr_alter_on_an_in_memory_crr_is_refused() {
        let db = Db::open_crsqlite(DbOpenOptions {
            storage: DbStorage::Memory,
            journal_mode_wal: false,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();
        migrate(&db, schema(SYNCED_BASE_STEPS, widgets_synced))
            .await
            .unwrap();
        hypr_crsqlite::as_crr(db.pool(), "widgets").await.unwrap();

        let error = migrate(&db, schema(CRR_ALTER_STEPS, widgets_synced))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("in-memory"), "{error}");
        assert_eq!(applied_versions(&db).await, vec![20260415030101]);
        db.close().await;
    }

    /// Audit finding (2026-11-04, muse seat): a connection checked out by
    /// another caller during the window would keep the pre-alter schema, so
    /// the window refuses to open rather than diverge silently.
    #[tokio::test]
    async fn crr_alter_is_refused_while_another_connection_is_checked_out() {
        let (_dir, db) = open_synced_widgets().await;
        let held = db.pool().acquire().await.unwrap();

        let error = migrate(&db, schema(CRR_ALTER_STEPS, widgets_synced))
            .await
            .unwrap_err();
        drop(held);

        assert!(error.to_string().contains("checked out"), "{error}");
        assert_eq!(applied_versions(&db).await, vec![20260415030101]);
        assert_eq!(
            widget_columns(&db).await,
            vec!["id".to_string(), "name".to_string()]
        );
        db.close().await;
    }

    /// On an empty table SQLite accepts `ADD COLUMN ... NOT NULL` without a
    /// DEFAULT, and `crsql_commit_alter` is what rejects it.
    #[tokio::test]
    async fn failed_crr_alter_commit_leaves_the_crr_intact() {
        let (_dir, db) = open_synced_widgets().await;

        let error = migrate(&db, schema(FAILING_CRR_ALTER_STEPS, widgets_synced))
            .await
            .unwrap_err();

        assert_failed_alter_left_the_crr_intact(&db, error).await;
        db.close().await;
    }

    /// With a row present SQLite itself rejects the same DDL.
    #[tokio::test]
    async fn failed_crr_alter_ddl_leaves_the_crr_intact() {
        let (_dir, db) = open_synced_widgets().await;
        insert_widget(&db, "w0").await;

        let error = migrate(&db, schema(FAILING_CRR_ALTER_STEPS, widgets_synced))
            .await
            .unwrap_err();

        assert_failed_alter_left_the_crr_intact(&db, error).await;
        db.close().await;
    }
}
