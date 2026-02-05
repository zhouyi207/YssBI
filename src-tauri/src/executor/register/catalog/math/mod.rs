pub mod math;

#[cfg(test)]
mod test;

use crate::executor::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    math::register(registry);
}
