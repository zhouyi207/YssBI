pub mod debug;

#[cfg(test)]
mod test;

use crate::graph::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    debug::register(registry);
}
