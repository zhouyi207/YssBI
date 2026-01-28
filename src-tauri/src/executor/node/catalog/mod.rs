pub mod internal;
pub mod function;
pub mod control;
pub mod debug;
pub mod math;
pub mod variable;
pub mod data;
pub mod visualization;

use super::registry::NodeRegistry;

pub fn register_builtin_nodes(registry: &NodeRegistry) {
    internal::register(registry);
    function::register(registry);
    control::register(registry);
    debug::register(registry);
    math::register(registry);
    variable::register(registry);
    data::register(registry);
    visualization::register(registry);
}
