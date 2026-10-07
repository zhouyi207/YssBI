//! Designed-experiment interfaces, separate from quality monitoring.
use super::*;
mod design;
mod models;
mod range;
pub(super) fn implemented(id: &str) -> bool {
    matches!(
        id,
        "yssbi.statistics.doe.range_analysis"
            | "yssbi.statistics.doe.family"
            | "yssbi.statistics.doe.orthogonal"
            | "yssbi.statistics.doe.uniform_design"
            | "yssbi.statistics.doe.response_surface"
            | "yssbi.statistics.doe.dose_response"
    )
}
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    range::append(fragment)?;
    design::append(fragment)?;
    models::append(fragment)
}
