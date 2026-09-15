use std::sync::Arc;

use super::identity::PlanProvenance;
use super::model::ExecutionPlan;
use super::parameter::PlanParameterBundle;

#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionPlanPackage {
    plan: Arc<ExecutionPlan>,
    parameters: Arc<PlanParameterBundle>,
    provenance: PlanProvenance,
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
