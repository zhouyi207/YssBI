pub mod internal;
pub mod function;
pub mod control;
pub mod debug;
pub mod math;
pub mod variable;
pub mod data;
pub mod visualization;

use super::definition::NodeDefinition;

pub fn get_all_builtin_nodes() -> Vec<NodeDefinition> {
    let mut nodes = Vec::new();
    
    nodes.extend(internal::get_nodes());
    nodes.extend(function::get_nodes());
    nodes.extend(control::get_nodes());
    nodes.extend(debug::get_nodes());
    nodes.extend(math::get_nodes());
    nodes.extend(variable::get_nodes());
    nodes.extend(data::get_nodes());
    nodes.extend(visualization::get_nodes());
    
    nodes
}
