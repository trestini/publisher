use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use crate::{
    error::AppError,
    features::events::dtos::{EventRequest, EventResponse},
    state::AppState,
};
use amqprs::{
    Ack, BasicProperties, Cancel, CloseChannel, FieldTable, FieldValue, Nack, Return, ShortStr,
    callbacks::ChannelCallback,
    channel::{
        BasicPublishArguments, ConfirmSelectArguments,
    },
};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use opentelemetry::trace::TraceContextExt as _;
use tokio::sync::oneshot;
use tracing::instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Callback for publisher confirms
// ---------------------------------------------------------------------------

struct ConfirmCallback {
    ack_tx: Option<oneshot::Sender<Result<(), amqprs::error::Error>>>,
    publish_start: Arc<Mutex<Option<std::time::Instant>>>,
    exchange: String,
}

impl ChannelCallback for ConfirmCallback {
    fn close<'a, 'b, 'async_trait>(
        &'a mut self,
        _channel: &'b amqprs::channel::Channel,
        _close: CloseChannel,
    ) -> Pin<Box<dyn Future<Output = Result<(), amqprs::error::Error>> + Send + 'async_trait>>
    where
        Self: 'async_trait,
        'a: 'async_trait,
        'b: 'async_trait,
    {
        Box::pin(async { Ok(()) })
    }

    fn cancel<'a, 'b, 'async_trait>(
        &'a mut self,
        _channel: &'b amqprs::channel::Channel,
        _cancel: Cancel,
    ) -> Pin<Box<dyn Future<Output = Result<(), amqprs::error::Error>> + Send + 'async_trait>>
    where
        Self: 'async_trait,
        'a: 'async_trait,
        'b: 'async_trait,
    {
        Box::pin(async { Ok(()) })
    }

    fn flow<'a, 'b, 'async_trait>(
        &'a mut self,
        _channel: &'b amqprs::channel::Channel,
        _active: bool,
    ) -> Pin<Box<dyn Future<Output = Result<bool, amqprs::error::Error>> + Send + 'async_trait>>
    where
        Self: 'async_trait,
        'a: 'async_trait,
        'b: 'async_trait,
    {
        Box::pin(async { Ok(true) })
    }

    fn publish_ack<'a, 'b, 'async_trait>(
        &'a mut self,
        _channel: &'b amqprs::channel::Channel,
        _ack: Ack,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'async_trait>>
    where
        Self: 'async_trait,
        'a: 'async_trait,
        'b: 'async_trait,
    {
        let elapsed_ns = self
            .publish_start
            .lock()
            .unwrap()
            .take()
            .map(|t| t.elapsed().as_nanos())
            .unwrap_or(0);
        let tx = self.ack_tx.take();
        let exchange = self.exchange.clone();
        Box::pin(async move {


            let elapsed_secs = elapsed_ns as f64 / 1_000_000_000.0;
            let histogram = metrics::histogram!(
                crate::infra::metrics::PROM_PIPELINE_PUBLISH_DURATION,
                "exchange" => exchange,
                "service" => "publisher",
            );
            histogram.record(elapsed_secs);

            if let Some(tx) = tx {
                let _ = tx.send(Ok(()));
            }
        })
    }

    fn publish_nack<'a, 'b, 'async_trait>(
        &'a mut self,
        _channel: &'b amqprs::channel::Channel,
        _nack: Nack,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'async_trait>>
    where
        Self: 'async_trait,
        'a: 'async_trait,
        'b: 'async_trait,
    {
        let tx = self.ack_tx.take();
        Box::pin(async move {
            if let Some(tx) = tx {
                let _ = tx.send(Err(amqprs::error::Error::ChannelUseError(
                    "publish was nacked by broker".into(),
                )));
            }
        })
    }

    fn publish_return<'a, 'b, 'async_trait>(
        &'a mut self,
        _channel: &'b amqprs::channel::Channel,
        _ret: Return,
        _basic_properties: BasicProperties,
        _content: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'async_trait>>
    where
        Self: 'async_trait,
        'a: 'async_trait,
        'b: 'async_trait,
    {
        Box::pin(async {})
    }
}

// ---------------------------------------------------------------------------
// Publish error type (distinguishes nack from other failures)
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum PublishError {
    Invalid(anyhow::Error),
    Nack(anyhow::Error),
}

impl From<PublishError> for AppError {
    fn from(e: PublishError) -> Self {
        match e {
            PublishError::Invalid(e) | PublishError::Nack(e) => AppError::UnexpectedError(e),
        }
    }
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

#[instrument(
    name = "ingest_event_handler",
    skip_all,
    fields(request_id = %"TODO_UUID", event_id = tracing::field::Empty)
)]
pub async fn ingest_event(
    State(state): State<AppState>,
    Json(mut payload): Json<EventRequest>,
) -> Result<impl IntoResponse, AppError> {
    let event_id = payload
        .event_id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Ensure the published message always carries the (possibly generated) event_id
    payload.event_id = Some(event_id.clone());

    tracing::Span::current().record("event_id", &event_id);

    let cx = tracing::Span::current().context();
    let trace_id = cx.span().span_context().trace_id().to_string();

    // -- inject W3C trace context into AMQP headers -------------------------

    let mut trace_carrier = std::collections::HashMap::new();
    opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&cx, &mut trace_carrier);
    });

    let mut amqp_headers = FieldTable::new();
    for (key, value) in &trace_carrier {
        let k = ShortStr::try_from(key.as_str())
            .expect("trace header key fits in short string");
        amqp_headers.insert(k, FieldValue::from(value.as_str()));
    }

    // -- publish & confirm ---------------------------------------------------

    let publish_result: Result<(), PublishError> = async {
        let content = serde_json::to_vec(&payload)
            .map_err(|e| PublishError::Invalid(e.into()))?;

        let channel = state
            .rmq
            .open_channel(None)
            .await
            .map_err(|e| PublishError::Invalid(e.into()))?;

        // -- set up publisher confirms ---------------------------------------

        let publish_start = Arc::new(Mutex::new(None));
        let (ack_tx, ack_rx) = oneshot::channel();
        let exchange = "ALTPAY:PAYIN".to_string();
        let callback = ConfirmCallback {
            ack_tx: Some(ack_tx),
            publish_start: Arc::clone(&publish_start),
            exchange,
        };
        channel
            .register_callback(callback)
            .await
            .map_err(|e| PublishError::Invalid(e.into()))?;

        channel
            .confirm_select(ConfirmSelectArguments::default())
            .await
            .map_err(|e| PublishError::Invalid(e.into()))?;

        // -- publish ---------------------------------------------------------

        let props = BasicProperties::default()
            .with_content_type("application/json")
            .with_persistence(true)
            .with_headers(amqp_headers)
            .finish();
        let pub_args = BasicPublishArguments::new("ALTPAY:PAYIN", "events.payin");
        *publish_start.lock().unwrap() = Some(std::time::Instant::now());
        channel
            .basic_publish(props, content, pub_args)
            .await
            .map_err(|e| PublishError::Invalid(e.into()))?;

        // -- wait for broker confirmation ------------------------------------

        ack_rx
            .await
            .map_err(|_| {
                PublishError::Invalid(anyhow::anyhow!("publish confirm channel closed"))
            })?
            .map_err(|e| PublishError::Nack(e.into()))?;

        Ok(())
    }
    .await;

    // -- record metrics based on result --------------------------------------

    match &publish_result {
        Ok(()) => {
            tracing::info!(event_id = %event_id, "Event published successfully");

            let counter = metrics::counter!(
                crate::infra::metrics::PROM_PIPELINE_EVENTS_PUBLISHED_TOTAL,
                "service" => "publisher",
            );
            counter.increment(1);
        }
        Err(PublishError::Nack(_)) => {
            tracing::error!(event_id = %event_id, "Failed to publish event");

            let counter = metrics::counter!(
                crate::infra::metrics::PROM_PIPELINE_EVENTS_FAILED_TOTAL,
                "service" => "publisher",
                "reason" => "nack",
            );
            counter.increment(1);
        }
        Err(PublishError::Invalid(_)) => {
            tracing::error!(event_id = %event_id, "Failed to publish event");

            let counter = metrics::counter!(
                crate::infra::metrics::PROM_PIPELINE_EVENTS_FAILED_TOTAL,
                "service" => "publisher",
                "reason" => "invalid",
            );
            counter.increment(1);
        }
    }

    // -- propagate any error ------------------------------------------------

    publish_result?;

    let response = EventResponse {
        trace_id,
        event_id,
    };

    Ok((StatusCode::ACCEPTED, Json(response)))
}
