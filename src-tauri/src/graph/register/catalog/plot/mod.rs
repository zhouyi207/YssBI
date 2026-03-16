mod scatter;
mod line;
mod ecdf;
mod kde;
mod histogram;
mod correlation_plot;
mod correlogram;

use crate::graph::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    scatter::register(registry);
    line::register(registry);
    ecdf::register(registry);
    kde::register(registry);
    histogram::register(registry);
    correlation_plot::register(registry);
    correlogram::register(registry);
}
