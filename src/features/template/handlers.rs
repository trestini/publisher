use crate::{
    error::AppError,
    features::template::{
        db::FlagsRepository,
        dtos::{CreateFlagRequest, EvaluationResult},
        models::{EvaluationContext, Flag, Rule},
    },
    state::AppState,
};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use tracing::instrument;
use tracing::{debug, info};

pub async fn create_flag(
    State(state): State<AppState>,
    Json(payload): Json<CreateFlagRequest>,
) -> Result<impl IntoResponse, AppError> {
    let repo = FlagsRepository::new(state.db);
    let flag = repo.create(&payload.key, &payload.name).await?;

    Ok((StatusCode::CREATED, Json(flag)))
}

#[instrument(
    name = "get_flag_handler",
    skip(state),
    fields(request_id = %"TODO_UUID")
)]
pub async fn get_flag(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Will recover flag with key {}", key);
    let flag: Flag = state.get_cached_flag(&key).await?;
    info!("Flag recovered");
    Ok((StatusCode::OK, Json(flag)))
}

pub async fn toggle_flag(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let repo = FlagsRepository::new(state.db);
    let result = repo.toggle_is_enabled(&key).await?;

    let flag = result.ok_or(AppError::NotAvailableError(key.clone()))?;

    state.flags_cache.remove(&key);

    Ok((StatusCode::OK, Json(flag)))
}

pub async fn check_flag(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(payload): Json<EvaluationContext>,
) -> Result<impl IntoResponse, AppError> {
    let flag = state.get_cached_flag(&key).await?;

    let result = flag.evaluate(&payload);

    let ret = EvaluationResult {
        key: String::from(key),
        result,
    };

    Ok((StatusCode::OK, Json(ret)))
}

pub async fn update_rules(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(payload): Json<Vec<Rule>>,
) -> Result<impl IntoResponse, AppError> {
    let repo = FlagsRepository::new(state.db);
    let updated = repo.set_rules(&key, payload).await?;
    state.flags_cache.remove(&key);
    Ok((StatusCode::OK, Json(updated)))
}
