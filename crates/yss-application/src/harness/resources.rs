//! Project-owned reference lookup on the blocking pool, under Core turn cancellation.

use crate::session::ApplicationState;
use yss_harness_contract::{
    AgentFuture, CancellationToken, CapabilityFailure, CapabilityFailureCode,
    HarnessResourceReference, HarnessResourceResolverPort, ProjectResourceRef,
    ProjectSessionBinding,
};

pub struct ApplicationResourceResolver(pub ApplicationState);

impl HarnessResourceResolverPort for ApplicationResourceResolver {
    fn resolve<'a>(
        &'a self,
        project: &'a ProjectSessionBinding,
        resources: &'a [ProjectResourceRef],
        cancellation: CancellationToken,
    ) -> AgentFuture<'a, Result<Vec<HarnessResourceReference>, CapabilityFailure>> {
        let application = self.0.clone();
        let project = project.clone();
        let resources = resources.to_vec();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                if cancellation.is_cancelled() {
                    return Err(CapabilityFailure::new(CapabilityFailureCode::Cancelled));
                }
                let result = application.resolve_harness_resources(&project, &resources);
                if cancellation.is_cancelled() {
                    return Err(CapabilityFailure::new(CapabilityFailureCode::Cancelled));
                }
                result
            })
            .await
            .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::InternalFailure))?
        })
    }
}
