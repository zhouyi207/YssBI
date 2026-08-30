#[derive(Debug, thiserror::Error)]
#[error("graph materialization invariant failed")]
pub struct GraphMaterializationError;

impl GraphMaterializationError {
    pub(crate) const fn invariant() -> Self {
        Self
    }
}
