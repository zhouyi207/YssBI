pub mod operators;

use crate::nodes::definition::NodeDefinition;

pub fn get_nodes() -> Vec<NodeDefinition> {
    let mut nodes = Vec::new();
    nodes.extend(operators::get_nodes());
    nodes
}
