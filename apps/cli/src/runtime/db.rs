use std::path::Path;

use hypr_db3::Db3;

const SQLITECLOUD_URL: &str = "SQLITECLOUD_URL";

pub async fn connect(path: impl AsRef<Path>) -> Result<Db3, Box<dyn std::error::Error>> {
    match std::env::var(SQLITECLOUD_URL).ok() {
        Some(url) => {
            let db = Db3::connect_local(path).await?;
            db.cloudsync_network_init(&url).await?;
            Ok(db)
        }
        None => {
            let db = Db3::connect_local_plain(path).await?;
            Ok(db)
        }
    }
}
