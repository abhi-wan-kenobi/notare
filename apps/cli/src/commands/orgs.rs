use sqlx::SqlitePool;

use crate::error::{CliError, CliResult};

pub async fn list(pool: &SqlitePool) -> CliResult<()> {
    let orgs = hypr_db_app::list_organizations(pool)
        .await
        .map_err(|e| CliError::operation_failed("list organizations", e.to_string()))?;
    for org in &orgs {
        println!("{}\t{}", org.id, org.name);
    }
    Ok(())
}

pub async fn add(pool: &SqlitePool, name: &str) -> CliResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    hypr_db_app::insert_organization(pool, &id, name)
        .await
        .map_err(|e| CliError::operation_failed("insert organization", e.to_string()))?;
    println!("{id}");
    Ok(())
}

pub async fn show(pool: &SqlitePool, id: &str) -> CliResult<()> {
    match hypr_db_app::get_organization(pool, id).await {
        Ok(Some(org)) => {
            println!("id: {}", org.id);
            println!("name: {}", org.name);
            println!("created_at: {}", org.created_at);

            match hypr_db_app::list_events_by_org(pool, id).await {
                Ok(events) if !events.is_empty() => {
                    println!();
                    println!("recent events:");
                    for event in &events {
                        let date = if event.started_at.len() >= 16 {
                            &event.started_at[..16]
                        } else {
                            &event.started_at
                        };
                        let date = date.replace('T', " ");
                        println!("  {}  {}", date, event.title);
                    }
                }
                _ => {}
            }

            Ok(())
        }
        Ok(None) => Err(CliError::msg(format!("organization '{id}' not found"))),
        Err(e) => Err(CliError::operation_failed("query", e.to_string())),
    }
}

pub async fn rm(pool: &SqlitePool, id: &str) -> CliResult<()> {
    hypr_db_app::delete_organization(pool, id)
        .await
        .map_err(|e| CliError::operation_failed("delete organization", e.to_string()))?;
    eprintln!("deleted {id}");
    Ok(())
}
