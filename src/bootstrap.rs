use std::{env::var, str::FromStr, time::Duration};

use amqprs::{
    channel::{ExchangeDeclareArguments, ExchangeType},
    connection::{Connection, OpenConnectionArguments},
};
use anyhow::Context;
use sqlx::ConnectOptions;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tracing::{info, instrument, log::LevelFilter};

use crate::state::AppState;

#[instrument(
    name = "bootstrap",
    fields(request_id = %"TODO_UUID")
)]
pub async fn start() -> anyhow::Result<AppState> {
    let db_url = var("DATABASE_URL").expect("DATABASE URL must be set");

    let db_opts = PgConnectOptions::from_str(&db_url)
        .context("Failed to set pg connection options")?
        .log_statements(LevelFilter::Info)
        .log_slow_statements(LevelFilter::Warn, Duration::from_secs(1));

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(3))
        .connect_with(db_opts)
        .await
        .context("Failed to connect with database")?;

    info!("Database Connection configured");

    let rmq_host = var("RABBITMQ_HOST").unwrap_or_else(|_| "localhost".into());
    let rmq_port = var("RABBITMQ_PORT")
        .unwrap_or_else(|_| "5672".into())
        .parse()
        .context("RABBITMQ_PORT must be a valid u16")?;
    let rmq_user = var("RABBITMQ_USER").unwrap_or_else(|_| "guest".into());
    let rmq_pass = var("RABBITMQ_PASS").unwrap_or_else(|_| "guest".into());

    let rmq_args = OpenConnectionArguments::new(&rmq_host, rmq_port, &rmq_user, &rmq_pass);
    let rmq_conn = Connection::open(&rmq_args)
        .await
        .context("Failed to connect to RabbitMQ")?;

    info!("RabbitMQ Connection configured");

    // -- declare exchanges ---------------------------------------------------

    let chan = rmq_conn
        .open_channel(None)
        .await
        .context("Failed to open channel for exchange declaration")?;

    let exchange_names = ["ALTPAY:PAYIN", "DLX__ALTPAY:PAYIN"];
    for name in &exchange_names {
        let args = ExchangeDeclareArguments::of_type(name, ExchangeType::Topic)
            .durable(true)
            .finish();
        chan.exchange_declare(args)
            .await
            .with_context(|| format!("Failed to declare exchange {name}"))?;
        info!(exchange = %name, "Exchange declared");
    }

    let _ = chan.close().await;

    Ok(AppState::new(pool, rmq_conn))
}
