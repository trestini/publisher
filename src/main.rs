use anyhow::Context;
use opentelemetry_sdk::trace::SdkTracerProvider;
use std::{net::SocketAddr, process::ExitCode, time::Duration};
use tracing::{error, info, info_span};

use dotenvy::dotenv;
use app::{
    bootstrap::{self},
    infra::telemetry::init_telemetry,
    router,
};
use tokio::{net::TcpListener, time::sleep};

use tracing_opentelemetry::OpenTelemetrySpanExt;

#[tokio::main]
async fn main() -> ExitCode {
    println!("Starting boot process...");
    dotenv().ok();

    let trace_provider = match init_telemetry() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("CRITICAL ERROR: Failed to start telemetry: {e}");
            return ExitCode::FAILURE;
        }
    };

    info!("Application telemetry succesfully started");

    let execution_result;

    {
        let root_span = info_span!("main_telemetry_span");
        let _guard = root_span.enter();

        execution_result = run_app(trace_provider.clone()).await;

        match &execution_result {
            Ok(_) => {
                info!("Application stopped succesfully");
                root_span.set_status(opentelemetry::trace::Status::Ok);
            }
            Err(err) => {
                error!(error.msg = %err, error.details = ?err, "Fatal Startup Error: {err:#}");
                root_span.set_status(opentelemetry::trace::Status::error(format!("{err}")));
            }
        }
    }

    info!("Flushing traces before exit");
    let _ = trace_provider.shutdown();

    sleep(Duration::from_secs(1)).await;
    println!("... EOL");

    if execution_result.is_err() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

async fn run_app(trace_provider: SdkTracerProvider) -> anyhow::Result<()> {
    let app_state = bootstrap::start()
        .await?;

    info!("Boot process completed");
    let app = router::setup_router(app_state);
    info!("Routes configured");

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Application HTTP Handler listening on {}", addr);

    let listener = TcpListener::bind(addr)
        .await
        .context(format!("Unable to bind {:?}", addr))?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(trace_provider))
        .await?;

    Ok(())
}

async fn shutdown_signal(provider: SdkTracerProvider) {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("Where is CTRL+C?");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install unix signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {}
    }

    info!("Shutting down, flushing traces");

    let _ = provider.shutdown();
}
