use std::sync::Arc;

use crate::database::runtime::DatabaseRuntimeSession;
use crate::execution::plan::PlanProjectSessionId;
use crate::execution::resource_preparation::ResourceProviderFactory;

pub fn database_resource_provider_factory(
    database_session: Arc<DatabaseRuntimeSession>,
    project_session: PlanProjectSessionId,
) -> ResourceProviderFactory {
    ResourceProviderFactory::new(project_session.as_str().into(), Some(database_session))
}
