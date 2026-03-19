use sqlx::SqlitePool;

use crate::error::{CliError, CliResult};

pub async fn list(pool: &SqlitePool) -> CliResult<()> {
    let humans = hypr_db_app::list_humans(pool)
        .await
        .map_err(|e| CliError::operation_failed("list humans", e.to_string()))?;
    for h in &humans {
        println!("{}\t{}\t{}", h.id, h.name, h.email);
    }
    Ok(())
}

pub async fn add(
    pool: &SqlitePool,
    name: &str,
    email: Option<&str>,
    org: Option<&str>,
    title: Option<&str>,
) -> CliResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    hypr_db_app::insert_human(
        pool,
        &id,
        name,
        email.unwrap_or(""),
        org.unwrap_or(""),
        title.unwrap_or(""),
    )
    .await
    .map_err(|e| CliError::operation_failed("insert human", e.to_string()))?;
    println!("{id}");
    Ok(())
}

pub async fn show(pool: &SqlitePool, id: &str) -> CliResult<()> {
    match hypr_db_app::get_human(pool, id).await {
        Ok(Some(h)) => {
            println!("id: {}", h.id);
            println!("name: {}", h.name);
            println!("email: {}", h.email);
            println!("org_id: {}", h.org_id);
            println!("job_title: {}", h.job_title);
            println!("created_at: {}", h.created_at);

            match hypr_db_app::list_events_by_human(pool, id).await {
                Ok(events) if !events.is_empty() => {
                    println!();
                    println!("recent events:");
                    for event in events.iter().take(10) {
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
        Ok(None) => Err(CliError::msg(format!("human '{id}' not found"))),
        Err(e) => Err(CliError::operation_failed("query", e.to_string())),
    }
}

pub async fn rm(pool: &SqlitePool, id: &str) -> CliResult<()> {
    hypr_db_app::delete_human(pool, id)
        .await
        .map_err(|e| CliError::operation_failed("delete human", e.to_string()))?;
    eprintln!("deleted {id}");
    Ok(())
}
