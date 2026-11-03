use std::path::Path;

use hypr_db_core::{Db, DbOpenOptions, DbStorage};

use crate::error::OpenAppDbError;

pub(crate) async fn open_app_db(db_path: &Path) -> Result<Db, OpenAppDbError> {
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Local(db_path),
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await?;

    hypr_db_app::prepare_schema(&db).await?;

    Ok(db)
}
