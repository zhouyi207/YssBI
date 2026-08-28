use std::sync::Arc;

use crate::database::runtime::DatabaseRuntimeSession;
use crate::execution::plan::identity::PlanProjectSessionId;
use crate::execution::resource_preparation::ResourceProviderFactory;

pub(crate) fn database_resource_provider_factory(
    _database_session: Arc<DatabaseRuntimeSession>,
    project_session: PlanProjectSessionId,
) -> ResourceProviderFactory {
    ResourceProviderFactory::new(project_session.as_str().into())
}
