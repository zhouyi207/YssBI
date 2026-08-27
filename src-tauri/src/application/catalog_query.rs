use crate::graph::resource_catalog::ResourceCatalogSnapshot;
use crate::node_system::catalog::LocalizedCatalog;

use super::execution::session_slot::{
    ApplicationState, SessionCaptureError, SessionRevalidationError,
};

#[derive(Debug, thiserror::Error)]
pub enum CatalogQueryApplicationError {
    #[error("application session capture failed")]
    SessionCapture(#[source] SessionCaptureError),
    #[error("application session changed during catalog query")]
    SessionChanged,
    #[error("catalog query failed")]
    Query,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogQueryResult {
    project_instance_id: Box<str>,
    registry_fingerprint: Box<str>,
    resource_publication_revision: u64,
    catalog: LocalizedCatalog,
}

impl CatalogQueryResult {
    pub(crate) fn new(
        project_instance_id: Box<str>,
        registry_fingerprint: Box<str>,
        resource_publication_revision: u64,
        catalog: LocalizedCatalog,
    ) -> Self {
        Self {
            project_instance_id,
            registry_fingerprint,
            resource_publication_revision,
            catalog,
        }
    }

    pub(crate) fn into_transport_parts(self) -> CatalogQueryResultParts {
        CatalogQueryResultParts {
            project_instance_id: self.project_instance_id,
            registry_fingerprint: self.registry_fingerprint,
            resource_publication_revision: self.resource_publication_revision,
            catalog: self.catalog,
        }
    }
}

pub(crate) struct CatalogQueryResultParts {
    project_instance_id: Box<str>,
    registry_fingerprint: Box<str>,
    resource_publication_revision: u64,
    catalog: LocalizedCatalog,
}

impl CatalogQueryResultParts {
    pub(crate) fn into_fields(self) -> (Box<str>, Box<str>, u64, LocalizedCatalog) {
        (
            self.project_instance_id,
            self.registry_fingerprint,
            self.resource_publication_revision,
            self.catalog,
        )
    }
}

pub fn localized_catalog(
    application: &ApplicationState,
    locale: &str,
) -> Result<CatalogQueryResult, CatalogQueryApplicationError> {
    let captured = application
        .capture_session()
        .map_err(CatalogQueryApplicationError::SessionCapture)?;
    let catalog = captured
        .graph()
        .localized_catalog(captured.graph().resource_catalog(), locale)
        .map_err(|_| CatalogQueryApplicationError::Query)?;
    application
        .revalidate_captured_session(&captured)
        .map_err(|error| match error {
            SessionRevalidationError::Unavailable(error) => {
                CatalogQueryApplicationError::SessionCapture(error)
            }
            SessionRevalidationError::Changed => CatalogQueryApplicationError::SessionChanged,
        })?;
    Ok(CatalogQueryResult::new(
        captured.project_instance_id().as_str().into(),
        "".into(),
        0,
        catalog,
    ))
}

pub fn compatible_catalog(
    application: &ApplicationState,
    locale: &str,
    _source: &ResourceCatalogSnapshot,
) -> Result<CatalogQueryResult, CatalogQueryApplicationError> {
    localized_catalog(application, locale)
}
