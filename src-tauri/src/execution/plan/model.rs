use super::identity::PlanSourceIdentity;
use super::observation::{PlanObservationIntent, ValueRef};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanOperation {
    source: PlanSourceIdentity,
    observation_intents: Box<[PlanObservationIntent]>,
    output: Option<ValueRef>,
}

impl PlanOperation {
    pub fn new(
        source: PlanSourceIdentity,
        observation_intents: Box<[PlanObservationIntent]>,
        output: Option<ValueRef>,
    ) -> Self {
        Self {
            source,
            observation_intents,
            output,
        }
    }

    pub fn source(&self) -> &PlanSourceIdentity {
        &self.source
    }

    pub fn observation_intents(&self) -> &[PlanObservationIntent] {
        &self.observation_intents
    }

    pub const fn output(&self) -> Option<ValueRef> {
        self.output
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionPlan {
    operations: Box<[PlanOperation]>,
}

impl ExecutionPlan {
    pub fn new(operations: Box<[PlanOperation]>) -> Self {
        Self { operations }
    }

    pub fn empty() -> Self {
        Self {
            operations: Box::new([]),
        }
    }

    pub fn operations(&self) -> &[PlanOperation] {
        &self.operations
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionPlanAbi {
    signature: Box<str>,
}

impl FunctionPlanAbi {
    pub fn new(signature: Box<str>) -> Self {
        Self { signature }
    }

    pub fn signature(&self) -> &str {
        &self.signature
    }
}
