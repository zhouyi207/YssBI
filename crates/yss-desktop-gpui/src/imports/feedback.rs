//! Translate typed import failures without exposing source paths or connection details.
use yss_application::database::samples::{SampleError, SampleImportError};

pub(crate) const IMPORT_FAILED: &str = "native.imports.failed";

pub(super) fn sample_failure(error: &SampleError) -> &'static str {
    match error {
        SampleError::InvalidCatalog | SampleError::CatalogDecode(_) => {
            "importModal.samples.errors.sample_catalog_invalid"
        }
        SampleError::NotFound => "importModal.samples.errors.sample_not_found",
        SampleError::VersionMismatch => "importModal.samples.errors.sample_version_mismatch",
        SampleError::Integrity => "importModal.samples.errors.sample_integrity_failed",
        SampleError::InvalidPath | SampleError::Unavailable(_) | SampleError::Decode(_) => {
            "importModal.samples.errors.sample_resource_unavailable"
        }
    }
}

pub(crate) fn sample_import_failure(error: &SampleImportError) -> &'static str {
    match error {
        SampleImportError::Sample(error) => sample_failure(error),
        // A failed refresh can follow a committed import: retain the existing recovery guidance.
        SampleImportError::Database(_) => IMPORT_FAILED,
    }
}
