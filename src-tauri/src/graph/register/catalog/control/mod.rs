pub mod control;

#[cfg(test)]
mod test;

use crate::graph::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    control::register(registry);
}
