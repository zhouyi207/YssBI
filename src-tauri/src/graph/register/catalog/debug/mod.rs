pub mod debug;
mod view_nodes;

#[cfg(test)]
mod test;

use crate::graph::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    debug::register(registry);
    view_nodes::register(registry);
}
