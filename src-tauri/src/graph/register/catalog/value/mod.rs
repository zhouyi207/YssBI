pub mod constants;
pub mod variables;
pub mod convert;
pub mod call;

#[cfg(test)]
mod test;

use crate::graph::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    constants::register(registry);
    variables::register(registry);
    convert::register(registry);
    call::register(registry);
}