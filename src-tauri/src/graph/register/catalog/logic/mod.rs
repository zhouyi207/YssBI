pub mod logic;

#[cfg(test)]
mod test;

use crate::graph::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    logic::register(registry);
}
