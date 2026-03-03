mod scatter;

use crate::graph::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    scatter::register(registry);
}
