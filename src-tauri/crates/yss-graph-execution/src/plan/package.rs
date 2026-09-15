use std::sync::Arc;

use super::identity::PlanProvenance;
use super::model::ExecutionPlan;
use super::parameter::CompiledParameterBundle;

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledExecutionPackage {
    plan: Arc<ExecutionPlan>,
    parameters: Arc<CompiledParameterBundle>,
    provenance: PlanProvenance,
}

impl CompiledExecutionPackage {
    pub fn new(
        plan: Arc<ExecutionPlan>,
        parameters: Arc<CompiledParameterBundle>,
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

    pub fn parameters(&self) -> &Arc<CompiledParameterBundle> {
        &self.parameters
    }

    pub fn provenance(&self) -> &PlanProvenance {
        &self.provenance
    }
}
