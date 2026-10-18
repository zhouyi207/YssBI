//! Designed-experiment interfaces, separate from quality monitoring.
use super::*;
mod design;
mod models;
mod range;
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    range::append(fragment)?;
    design::append(fragment)?;
    models::append(fragment)
}
