pub mod control;

#[cfg(test)]
mod test;

use crate::executor::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    control::register(registry);
}
