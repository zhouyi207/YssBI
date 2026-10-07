//! Shared capability input bounds and resource-identity validation.

use crate::CapabilityContractError;

pub const MAX_RESOURCE_ID_BYTES: usize = 1_024;

pub const MAX_CATALOG_QUERY_BYTES: usize = 256;

pub const MAX_LOCALE_BYTES: usize = 32;

pub const MAX_CATALOG_RESULTS: u16 = 100;

pub(crate) fn validate_resource_id(
    field: &'static str,
    value: &str,
) -> Result<(), CapabilityContractError> {
    if value.trim().is_empty() {
        return Err(CapabilityContractError::InvalidField(field));
    }
    if value.len() > MAX_RESOURCE_ID_BYTES {
        return Err(CapabilityContractError::FieldTooLong {
            field,
            maximum: MAX_RESOURCE_ID_BYTES,
        });
    }
    Ok(())
}
