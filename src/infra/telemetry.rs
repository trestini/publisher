use std::fmt::{self, Write as _};

use opentelemetry::{KeyValue, global};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    Resource,
    propagation::TraceContextPropagator,
    trace::{self},
};
use tracing::field::{Field, Visit};
use tracing_subscriber::{
    fmt::{
        FmtContext, FormatEvent, FormatFields,
        format::{DefaultFields, Writer},
    },
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

const TELEMETRY_APP_ID: &str = "publisher";

pub fn init_telemetry() -> Result<trace::SdkTracerProvider, Box<dyn std::error::Error>> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(
            std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".into()),
        )
        .build()?;

    let resource = Resource::builder()
        .with_attributes(vec![
            KeyValue::new("service.name", TELEMETRY_APP_ID),
            KeyValue::new("env", "production"),
        ])
        .build();

    let provider = trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    let tracer = opentelemetry::trace::TracerProvider::tracer(&provider, "sentinel-tracer");

    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "app=debug,tower_http=debug,axum::rejection=trace".into());

    let fmt_layer = tracing_subscriber::fmt::layer()
        .event_format(PublisherJsonFormatter)
        .fmt_fields(DefaultFields::new());

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(telemetry_layer)
        .init();

    Ok(provider)
}

// ---------------------------------------------------------------------------
// Custom JSON event formatter
// ---------------------------------------------------------------------------

struct PublisherJsonFormatter;

impl<S, N> FormatEvent<S, N> for PublisherJsonFormatter
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> fmt::Result {
        // Timestamp (RFC 3339)
        let timestamp = chrono::Utc::now().to_rfc3339();

        // Level
        let level = event.metadata().level().to_string();

        // Event fields — extract `message` and `event_id` from the event
        let mut message = String::new();
        let mut event_id = String::new();
        event.record(&mut EventFieldVisitor {
            message: &mut message,
            event_id: &mut event_id,
        });

        // Trace ID from the current tracing span's OTEL context
        let trace_id = current_trace_id();

        // Serialize as a single-line JSON object
        let output = serde_json::json!({
            "timestamp": timestamp,
            "level": level,
            "service": "publisher",
            "trace_id": trace_id,
            "event_id": event_id,
            "message": message,
        });
        let serialized = serde_json::to_string(&output).unwrap_or_default();
        write!(writer, "{serialized}")?;
        writeln!(writer)
    }
}

/// Returns the current OpenTelemetry trace ID as a 32-char hex string,
/// or all zeros if no span is active.
fn current_trace_id() -> String {
    use opentelemetry::trace::TraceContextExt as _;
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;

    tracing::Span::current()
        .context()
        .span()
        .span_context()
        .trace_id()
        .to_string()
}

/// Visitor that extracts `message` and (optionally) `event_id` fields
/// from a tracing event.
struct EventFieldVisitor<'a> {
    message: &'a mut String,
    event_id: &'a mut String,
}

impl<'a> Visit for EventFieldVisitor<'a> {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "message" => self.message.push_str(value),
            "event_id" => self.event_id.push_str(value),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        match field.name() {
            "message" => {
                // fmt::Arguments (used for format strings) implements Debug
                let _ = write!(self.message, "{value:?}");
            }
            "event_id" => {
                let _ = write!(self.event_id, "{value:?}");
            }
            _ => {}
        }
    }
}
