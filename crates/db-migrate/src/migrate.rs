use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use hypr_db_core::Db;
use sqlx::migrate::{
    AppliedMigration, Migrate, MigrateError as SqlxMigrateError, Migration, MigrationType,
};
use sqlx::{Executor, SqlSafeStr, Sqlite, SqliteConnection};

use crate::error::MigrateError;
use crate::schema::{DbSchema, MigrationScope, MigrationStep};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

struct DbMigrateConnection<'a> {
    db: &'a Db,
    // `None` after a CRR alter window closed the connection; the next use
    // acquires a fresh one, which sees the altered schema.
    conn: Option<sqlx::pool::PoolConnection<Sqlite>>,
    scopes_by_version: HashMap<i64, MigrationScope>,
}

impl<'a> DbMigrateConnection<'a> {
    fn new(
        db: &'a Db,
        conn: sqlx::pool::PoolConnection<Sqlite>,
        scopes_by_version: HashMap<i64, MigrationScope>,
    ) -> Self {
        Self {
            db,
            conn: Some(conn),
            scopes_by_version,
        }
    }

    async fn take_conn(&mut self) -> Result<sqlx::pool::PoolConnection<Sqlite>, sqlx::Error> {
        match self.conn.take() {
            Some(conn) => Ok(conn),
            None => self.db.pool().acquire().await,
        }
    }

    async fn conn(&mut self) -> Result<&mut SqliteConnection, sqlx::Error> {
        let conn = self.take_conn().await?;
        Ok(&mut **self.conn.insert(conn))
    }
}

pub(crate) async fn run_migrations(db: &Db, schema: DbSchema) -> Result<(), MigrateError> {
    let resolved = resolve_migrations(schema)?;
    let scopes_by_version = resolved
        .iter()
        .map(|(step, migration)| (migration.version, step.scope))
        .collect();
    let migrations: Vec<_> = resolved
        .into_iter()
        .map(|(_, migration)| migration)
        .collect();

    let conn = db.pool().acquire().await?;
    let mut conn = DbMigrateConnection::new(db, conn, scopes_by_version);
    run_direct(&migrations, &mut conn).await?;
    Ok(())
}

const MIGRATIONS_TABLE: &str = "_sqlx_migrations";

async fn run_direct<C>(migrations: &[Migration], conn: &mut C) -> Result<(), SqlxMigrateError>
where
    C: Migrate,
{
    conn.lock().await?;
    conn.ensure_migrations_table(MIGRATIONS_TABLE).await?;

    if let Some(version) = conn.dirty_version(MIGRATIONS_TABLE).await? {
        return Err(SqlxMigrateError::Dirty(version));
    }

    let applied_migrations = conn.list_applied_migrations(MIGRATIONS_TABLE).await?;
    validate_applied_migrations(&applied_migrations, migrations)?;

    let applied_migrations: HashMap<_, _> = applied_migrations
        .into_iter()
        .map(|migration| (migration.version, migration))
        .collect();

    for migration in migrations {
        if migration.migration_type.is_down_migration() {
            continue;
        }

        match applied_migrations.get(&migration.version) {
            Some(applied_migration) => {
                if migration.checksum != applied_migration.checksum {
                    return Err(SqlxMigrateError::VersionMismatch(migration.version));
                }
            }
            None => {
                conn.apply(MIGRATIONS_TABLE, migration).await?;
            }
        }
    }

    conn.unlock().await?;
    Ok(())
}

fn validate_applied_migrations(
    applied_migrations: &[AppliedMigration],
    migrations: &[Migration],
) -> Result<(), SqlxMigrateError> {
    let versions: HashSet<_> = migrations
        .iter()
        .map(|migration| migration.version)
        .collect();

    for applied_migration in applied_migrations {
        if !versions.contains(&applied_migration.version) {
            return Err(SqlxMigrateError::VersionMissing(applied_migration.version));
        }
    }

    Ok(())
}

fn resolve_migrations(
    schema: DbSchema,
) -> Result<Vec<(&'static MigrationStep, Migration)>, MigrateError> {
    let mut seen_versions = HashMap::new();
    let mut migrations = Vec::with_capacity(schema.steps.len());

    for step in schema.steps {
        validate_step(schema, step)?;

        let (version, description) = parse_step_id(step.id)?;

        if let Some(first_step_id) = seen_versions.insert(version, step.id) {
            return Err(MigrateError::DuplicateStepVersion {
                version,
                first_step_id,
                second_step_id: step.id,
            });
        }

        migrations.push((
            step,
            Migration::new(
                version,
                Cow::Borrowed(description),
                MigrationType::Simple,
                step.sql.into_sql_str(),
                step.sql.starts_with("-- no-transaction"),
            ),
        ));
    }

    migrations.sort_by_key(|(_, migration)| migration.version);
    Ok(migrations)
}

fn validate_step(schema: DbSchema, step: &MigrationStep) -> Result<(), MigrateError> {
    let MigrationScope::CrrAlter { table_name } = step.scope else {
        return Ok(());
    };

    if (schema.validate_crr_table)(table_name) {
        return Ok(());
    }

    Err(MigrateError::InvalidCrrStep {
        step_id: step.id,
        table_name,
    })
}

fn parse_step_id(step_id: &'static str) -> Result<(i64, &'static str), MigrateError> {
    let Some((version, description)) = step_id.split_once('_') else {
        return Err(MigrateError::InvalidStepId { step_id });
    };

    let version = version
        .parse::<i64>()
        .ok()
        .filter(|version| *version > 0)
        .ok_or(MigrateError::InvalidStepId { step_id })?;

    if description.is_empty() {
        return Err(MigrateError::InvalidStepId { step_id });
    }

    Ok((version, description))
}

impl Migrate for DbMigrateConnection<'_> {
    fn create_schema_if_not_exists<'e>(
        &'e mut self,
        schema_name: &'e str,
    ) -> BoxFuture<'e, Result<(), SqlxMigrateError>> {
        Box::pin(async move {
            <SqliteConnection as Migrate>::create_schema_if_not_exists(
                self.conn().await?,
                schema_name,
            )
            .await
        })
    }

    fn ensure_migrations_table<'e>(
        &'e mut self,
        table_name: &'e str,
    ) -> BoxFuture<'e, Result<(), SqlxMigrateError>> {
        Box::pin(async move {
            <SqliteConnection as Migrate>::ensure_migrations_table(self.conn().await?, table_name)
                .await
        })
    }

    fn dirty_version<'e>(
        &'e mut self,
        table_name: &'e str,
    ) -> BoxFuture<'e, Result<Option<i64>, SqlxMigrateError>> {
        Box::pin(async move {
            <SqliteConnection as Migrate>::dirty_version(self.conn().await?, table_name).await
        })
    }

    fn list_applied_migrations<'e>(
        &'e mut self,
        table_name: &'e str,
    ) -> BoxFuture<'e, Result<Vec<AppliedMigration>, SqlxMigrateError>> {
        Box::pin(async move {
            <SqliteConnection as Migrate>::list_applied_migrations(self.conn().await?, table_name)
                .await
        })
    }

    fn lock(&mut self) -> BoxFuture<'_, Result<(), SqlxMigrateError>> {
        Box::pin(async move { <SqliteConnection as Migrate>::lock(self.conn().await?).await })
    }

    fn unlock(&mut self) -> BoxFuture<'_, Result<(), SqlxMigrateError>> {
        Box::pin(async move { <SqliteConnection as Migrate>::unlock(self.conn().await?).await })
    }

    fn apply<'e>(
        &'e mut self,
        table_name: &'e str,
        migration: &'e Migration,
    ) -> BoxFuture<'e, Result<Duration, SqlxMigrateError>> {
        Box::pin(async move {
            let scope = self
                .scopes_by_version
                .get(&migration.version)
                .copied()
                .unwrap_or(MigrationScope::Plain);

            match scope {
                MigrationScope::Plain => {
                    <SqliteConnection as Migrate>::apply(self.conn().await?, table_name, migration)
                        .await
                }
                MigrationScope::CrrAlter {
                    table_name: crr_table,
                } => {
                    let start = Instant::now();
                    let db = self.db;
                    let mut conn = self.take_conn().await?;

                    let alter = db.begin_crr_alter_on(&mut conn, crr_table).await;
                    let Some(alter) = alter.map_err(SqlxMigrateError::Execute)? else {
                        let result =
                            <SqliteConnection as Migrate>::apply(&mut *conn, table_name, migration)
                                .await;
                        self.conn = Some(conn);
                        return result;
                    };

                    // Both ways out of the window close `conn`. A failed one
                    // rolls back with it — including this step's
                    // `_sqlx_migrations` row, so the step stays unapplied.
                    if let Err(error) = execute_migration(&mut conn, migration).await {
                        alter.abort(conn).await;
                        return Err(error);
                    }
                    alter
                        .commit(conn)
                        .await
                        .map_err(SqlxMigrateError::Execute)?;

                    let elapsed = start.elapsed();
                    update_execution_time(self.conn().await?, migration.version, elapsed).await?;

                    Ok(elapsed)
                }
            }
        })
    }

    fn revert<'e>(
        &'e mut self,
        table_name: &'e str,
        migration: &'e Migration,
    ) -> BoxFuture<'e, Result<Duration, SqlxMigrateError>> {
        Box::pin(async move {
            <SqliteConnection as Migrate>::revert(self.conn().await?, table_name, migration).await
        })
    }
}

async fn execute_migration(
    conn: &mut SqliteConnection,
    migration: &Migration,
) -> Result<(), SqlxMigrateError> {
    conn.execute(migration.sql.clone())
        .await
        .map_err(|err| SqlxMigrateError::ExecuteMigration(err, migration.version))?;

    sqlx::query(
        r#"
INSERT INTO _sqlx_migrations ( version, description, success, checksum, execution_time )
VALUES ( ?1, ?2, TRUE, ?3, -1 )
        "#,
    )
    .bind(migration.version)
    .bind(&*migration.description)
    .bind(&*migration.checksum)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

async fn update_execution_time(
    conn: &mut SqliteConnection,
    version: i64,
    elapsed: Duration,
) -> Result<(), SqlxMigrateError> {
    #[allow(clippy::cast_possible_truncation)]
    sqlx::query(
        r#"
UPDATE _sqlx_migrations
SET execution_time = ?1
WHERE version = ?2
        "#,
    )
    .bind(elapsed.as_nanos() as i64)
    .bind(version)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
