pub mod logic;

#[cfg(test)]
mod test;

use crate::executor::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    logic::register(registry);
}
