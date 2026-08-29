use crate::execution::plan::identity::PlanProjectSessionId;
use crate::execution::resource_preparation::ResourceProviderFactory;

pub(crate) fn database_resource_provider_factory(
    project_session: PlanProjectSessionId,
) -> ResourceProviderFactory {
    ResourceProviderFactory::from_project_session(project_session.as_str().into())
}
