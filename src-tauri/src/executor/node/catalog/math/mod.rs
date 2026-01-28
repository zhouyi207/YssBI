pub mod operators;

use crate::executor::node::registry::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    operators::register(registry);
}
