use crate::modules::service::Service;

use sqlx::{self, SqlitePool};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DBError {
    #[error("Unable to use database")]
    Sql(#[from] sqlx::Error),
}

pub async fn get_services(pool: &SqlitePool) -> Result<Vec<Service>, DBError> {
    let rows = sqlx::query!(
        r#"
            SELECT id, name, compose_name, repo_url, access_url, active, use_key FROM service
        "#
    )
    .fetch_all(pool)
    .await?;

    let result = rows
        .into_iter()
        .map(|row| Service {
            id: row.id,
            name: row.name,
            compose_name: row.compose_name,
            repo_url: row.repo_url,
            access_url: row.access_url,
            active: row.active,
            use_key: row.use_key,
        })
        .collect();

    Ok(result)
}

pub async fn get_service(pool: &SqlitePool, service_id: i64) -> Result<Service, DBError> {
    let result = sqlx::query_as!(
        Service,
        r#"
            SELECT id, name, compose_name, repo_url, access_url, active, use_key FROM service WHERE id = $1
        "#,
        service_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(result)
}

pub async fn new_service(pool: &SqlitePool, service: Service) -> Result<(), DBError> {
    sqlx::query!(
        "INSERT INTO service (name, compose_name, repo_url, access_url, active, use_key)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id",
        service.name,
        service.compose_name,
        service.repo_url,
        service.access_url,
        service.active,
        service.use_key,
    )
    .fetch_one(pool)
    .await?;
    Ok(())
}

pub async fn update_service(pool: &SqlitePool, id: i64, service: Service) -> Result<(), DBError> {
    sqlx::query!(
        "UPDATE service SET name = $1, compose_name = $2, repo_url = $3, access_url = $4, active = $5, use_key = $6  WHERE id = $7 RETURNING id",
        service.name,
        service.compose_name,
        service.repo_url,
        service.access_url,
        service.active,
        service.use_key,
        id,
    )
    .fetch_one(pool)
    .await?;
    Ok(())
}
