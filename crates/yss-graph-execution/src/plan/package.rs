use std::sync::Arc;

use super::identity::PlanProvenance;
use super::model::ExecutionPlan;
use super::parameter::PlanParameterBundle;

#[derive(Clone, Debug)]
pub struct ExecutionPlanPackage {
    plan: Arc<ExecutionPlan>,
    parameters: Arc<PlanParameterBundle>,
    provenance: PlanProvenance,
    pub(crate) functions: Option<Arc<crate::function_library::GraphFunctionLibrary>>,
}

impl ExecutionPlanPackage {
    pub fn new(
        plan: Arc<ExecutionPlan>,
        parameters: Arc<PlanParameterBundle>,
        provenance: PlanProvenance,
    ) -> Self {
        Self {
            plan,
            parameters,
            provenance,
            functions: None,
        }
    }

    pub fn plan(&self) -> &Arc<ExecutionPlan> {
        &self.plan
    }

    pub fn parameters(&self) -> &Arc<PlanParameterBundle> {
        &self.parameters
    }

    pub fn provenance(&self) -> &PlanProvenance {
        &self.provenance
    }
}
