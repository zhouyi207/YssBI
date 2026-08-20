pub mod analysis;
pub mod catalog;
pub(crate) mod compatibility;
pub mod compiler;
pub mod document;
mod id_allocator;
pub mod plan;
pub mod protocol;
pub mod registry;
pub mod runtime;

pub(crate) use id_allocator::allocate_nonzero_id;

#[cfg(test)]
pub mod testing;
