pub mod debug;

#[cfg(test)]
mod test;

use crate::executor::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    debug::register(registry);
}
