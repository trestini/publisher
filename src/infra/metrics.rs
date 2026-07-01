use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

/// Histogram: time from publish to broker ack, in seconds.
/// Attributes: exchange=<exchange_name>, service="publisher"
pub const PROM_PIPELINE_PUBLISH_DURATION: &str = "pipeline_publish_duration_seconds";

/// Counter: total messages successfully published (ack received).
/// Attributes: service="publisher"
pub const PROM_PIPELINE_EVENTS_PUBLISHED_TOTAL: &str = "pipeline_events_published_total";

/// Counter: total messages that failed to publish.
/// Attributes: service="publisher", reason="nack"|"invalid"
pub const PROM_PIPELINE_EVENTS_FAILED_TOTAL: &str = "pipeline_events_failed_total";

pub fn init_metrics() -> PrometheusHandle {
    let prom = PrometheusBuilder::new();
    prom.install_recorder()
        .expect("Prometheus metrics exporter failed to start")
}
