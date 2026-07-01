use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreateFlagRequest {
    pub key: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub key: String,
    pub result: bool,
}
