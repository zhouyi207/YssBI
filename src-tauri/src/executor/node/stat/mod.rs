pub mod t_test;

use crate::executor::node::registry::NodeRegistry;

pub fn register_all(registry: &NodeRegistry) {
    t_test::register(registry);
}
