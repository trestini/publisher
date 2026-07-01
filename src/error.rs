use axum::{Json, http::StatusCode, response::IntoResponse};
use serde_json::json;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Resource not available")]
    NotAvailableError(String),

    #[error("Unexpected Error")]
    UnexpectedError(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AppError::NotAvailableError(resource) => (
                StatusCode::NOT_FOUND,
                format!("Resource {} is not available", resource),
            ),
            AppError::UnexpectedError(e) => {
                let error_msg = format!("{:?}", e);
                tracing::error!(error_msg);

                (StatusCode::INTERNAL_SERVER_ERROR, error_msg)
            }
        };

        let body = Json(json!({
            "error": message
        }));

        (status, body).into_response()
    }
}
