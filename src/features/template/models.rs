use chrono::{DateTime, Utc};
use crc32fast::Hasher;
use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, types::Json};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct Flag {
    pub id: Uuid,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub is_enabled: bool,
    pub rules: Json<Vec<Rule>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct EvaluationContext {
    pub user_id: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Rule {
    TargetUsers { user_ids: Vec<String> },
    Percentage { rollout: u8 },
}

impl Flag {
    pub fn evaluate(&self, context: &EvaluationContext) -> bool {
        if !self.is_enabled {
            return false;
        }

        if self.rules.0.is_empty() {
            return true;
        }

        for rule in &self.rules.0 {
            match rule {
                Rule::TargetUsers { user_ids } => {
                    if let Some(uid) = &context.user_id {
                        if user_ids.contains(uid) {
                            return true;
                        }
                    }
                }
                Rule::Percentage { rollout } => {
                    if let Some(uid) = &context.user_id {
                        let hash_input = format!("{}:{}", uid, self.key);
                        let mut hasher = Hasher::new();
                        hasher.update(hash_input.as_bytes());
                        let checksum = hasher.finalize();

                        let bucket = checksum % 100;

                        if (bucket as u8) < *rollout {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }
}
