pub mod constants;
pub mod variables;
pub mod convert;

#[cfg(test)]
mod test;

use crate::executor::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    constants::register(registry);
    variables::register(registry);
    convert::register(registry);
}