pub mod math;
pub mod unary;

#[cfg(test)]
mod test;

use crate::graph::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    math::register(registry);
    unary::register(registry);
}
