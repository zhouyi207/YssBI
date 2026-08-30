use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use yss_tracing::LogLayer;

use super::dispatcher::DiagnosticsHub;
use super::rust_projection::log_record_sink;

/// Test-only adapter for exercising diagnostic projection with a local tracing
/// subscriber. Production tracing ownership lives entirely in `yss-tracing`.
pub(crate) struct RecentDiagnosticsLayer {
    inner: LogLayer,
}

impl RecentDiagnosticsLayer {
    pub(crate) fn new(hub: DiagnosticsHub) -> Self {
        Self {
            inner: LogLayer::new(log_record_sink(hub)),
        }
    }
}

impl<S> Layer<S> for RecentDiagnosticsLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, context: Context<'_, S>) {
        self.inner.on_event(event, context);
    }
}
