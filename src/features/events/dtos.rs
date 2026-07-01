use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRequest {
    pub partner_id: String,
    pub payload: serde_json::Value,
    #[serde(default)]
    pub event_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventResponse {
    pub trace_id: String,
    pub event_id: String,
}
