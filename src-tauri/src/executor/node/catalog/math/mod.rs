pub mod operators;
pub mod multi_output;
pub mod dynamic_add;

use crate::executor::node::registry::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    operators::register(registry);
    multi_output::register(registry);
    dynamic_add::register(registry);
}
