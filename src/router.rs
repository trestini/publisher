use crate::{
    features::events::handlers::ingest_event,
    infra::metrics::init_metrics,
    state::AppState,
};
use axum::{
    Router,
    extract::State,
    routing::{get, post},
};

pub fn setup_router(app_state: AppState) -> Router {
    let metrics_handler = init_metrics();

    Router::new()
        .route("/healthz", get(health_check))
        .route("/events", post(ingest_event))
//        .route("/flag", post(create_flag))
        .route(
            "/metrics",
            get(move || std::future::ready(metrics_handler.render())),
        )
        .with_state(app_state)
}

async fn health_check(State(_state): State<AppState>) -> &'static str {
    "OK"
}
