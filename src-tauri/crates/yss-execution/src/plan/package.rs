use std::sync::Arc;

use super::basis::PlanCompilationBasis;
use super::identity::{PlanProvenance, PlanResourceId, PlanResourceVersion};
use super::model::{ExecutionPlan, FunctionPlanAbi};
use super::parameter::CompiledParameterBundle;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledFunctionPlan {
    resource: PlanResourceId,
    version: PlanResourceVersion,
    plan: Arc<ExecutionPlan>,
    abi: Arc<FunctionPlanAbi>,
}

impl CompiledFunctionPlan {
    pub fn new(
        resource: PlanResourceId,
        version: PlanResourceVersion,
        plan: Arc<ExecutionPlan>,
        abi: Arc<FunctionPlanAbi>,
    ) -> Self {
        Self {
            resource,
            version,
            plan,
            abi,
        }
    }

    pub fn resource(&self) -> &PlanResourceId {
        &self.resource
    }

    pub fn version(&self) -> &PlanResourceVersion {
        &self.version
    }

    pub fn plan(&self) -> &Arc<ExecutionPlan> {
        &self.plan
    }

    pub fn abi(&self) -> &Arc<FunctionPlanAbi> {
        &self.abi
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledFunctionBundle {
    basis: PlanCompilationBasis,
    plans: Box<[CompiledFunctionPlan]>,
    recursion_limit: usize,
}

impl CompiledFunctionBundle {
    pub fn new(
        basis: PlanCompilationBasis,
        plans: Box<[CompiledFunctionPlan]>,
        recursion_limit: usize,
    ) -> Self {
        Self {
            basis,
            plans,
            recursion_limit,
        }
    }

    pub fn basis(&self) -> &PlanCompilationBasis {
        &self.basis
    }

    pub fn plans(&self) -> &[CompiledFunctionPlan] {
        &self.plans
    }

    pub const fn recursion_limit(&self) -> usize {
        self.recursion_limit
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledExecutionPackage {
    plan: Arc<ExecutionPlan>,
    functions: Arc<CompiledFunctionBundle>,
    parameters: Arc<CompiledParameterBundle>,
    provenance: PlanProvenance,
}

impl CompiledExecutionPackage {
    pub fn new(
        plan: Arc<ExecutionPlan>,
        functions: Arc<CompiledFunctionBundle>,
        parameters: Arc<CompiledParameterBundle>,
        provenance: PlanProvenance,
    ) -> Self {
        Self {
            plan,
            functions,
            parameters,
            provenance,
        }
    }

    pub fn plan(&self) -> &Arc<ExecutionPlan> {
        &self.plan
    }

    pub fn functions(&self) -> &Arc<CompiledFunctionBundle> {
        &self.functions
    }

    pub fn parameters(&self) -> &Arc<CompiledParameterBundle> {
        &self.parameters
    }

    pub fn provenance(&self) -> &PlanProvenance {
        &self.provenance
    }
}
