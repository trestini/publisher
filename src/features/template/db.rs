use std::time::Duration;
use tracing::info_span;

use sqlx::{PgPool, query_as, types::Json};
use tokio::time::sleep;
use tracing::{Instrument, instrument};

use anyhow::Context;

use crate::{
    error::AppError,
    features::template::models::{Flag, Rule},
};

#[derive(Clone, Debug)]
pub struct FlagsRepository {
    pool: PgPool,
}

impl FlagsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn set_rules(&self, key: &str, rules: Vec<Rule>) -> Result<Flag, AppError> {
        let flag = query_as!(
            Flag,
            r#"
            UPDATE flags set rules = $1 where key = $2
            RETURNING id, key, name, description, 
            is_enabled,
            rules as "rules: sqlx::types::Json<Vec<Rule>>", 
            created_at, updated_at
            "#,
            Json(rules) as _,
            key
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to set rules")?;

        Ok(flag)
    }

    pub async fn create(&self, key: &str, name: &str) -> Result<Flag, AppError> {
        let flag = query_as!(
            Flag,
            r#"
            INSERT into flags (key, name) values ($1, $2) 
            RETURNING id, key, name, description, 
            is_enabled,
            rules as "rules: sqlx::types::Json<Vec<Rule>>", 
            created_at, updated_at
            "#,
            key,
            name
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to create new flag")?;

        Ok(flag)
    }

    #[instrument(
        name = "flag-find_by_key",
        skip(self),
        fields(request_id = %"TODO_UUID")
    )]
    pub async fn find_by_key(&self, key: &str) -> Result<Option<Flag>, AppError> {
        sleep(Duration::from_secs(4)).await;
        let flag = query_as!(
            Flag,
            r#"
            SELECT id, key, name, description, is_enabled,
            rules as "rules: sqlx::types::Json<Vec<Rule>>", 
            created_at, updated_at from flags where key = $1
            "#,
            key
        )
        .fetch_optional(&self.pool)
        .instrument(info_span!("db_query", db.system = "postgresql"))
        .await
        .context(format!("Failed to get flag by key {}", key))?;

        Ok(flag)
    }

    pub async fn toggle_is_enabled(&self, key: &str) -> Result<Option<Flag>, AppError> {
        let flag = query_as!(
            Flag,
            r#"
            UPDATE flags set is_enabled = NOT is_enabled  where key = $1
            RETURNING id, key, name, description, 
            is_enabled, 
            rules as "rules: sqlx::types::Json<Vec<Rule>>", 
            created_at, updated_at
            "#,
            key
        )
        .fetch_optional(&self.pool)
        .await
        .context(format!("Failed to toggle flag with key {}", key))?;

        Ok(flag)
    }
}
