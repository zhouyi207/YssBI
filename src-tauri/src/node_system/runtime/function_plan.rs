use super::FunctionPlanProvider;
use crate::node_system::analysis::{
    ProjectSessionId, ResourceKey, ResourceVersion, ResourceVersionSet,
};
use crate::node_system::document::GraphResourcePath;
use crate::node_system::plan::{
    ExecutionPlan, FunctionPlanAbi, FunctionPlanHandle, PlanSourceFacts,
};
use crate::node_system::registry::RegistryFingerprint;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FunctionPlanKey {
    path: GraphResourcePath,
    version: ResourceVersion,
}

pub struct FunctionPlanStore {
    project_session_id: ProjectSessionId,
    recursion_limit: usize,
}

impl FunctionPlanStore {
    pub fn new(project_session_id: ProjectSessionId, recursion_limit: usize) -> Self {
        Self {
            project_session_id,
            recursion_limit: recursion_limit.max(1),
        }
    }

    pub fn generation(
        &self,
        registry_fingerprint: RegistryFingerprint,
        resource_versions: ResourceVersionSet,
        entries: Vec<(
            GraphResourcePath,
            ResourceVersion,
            Arc<ExecutionPlan>,
            Arc<FunctionPlanAbi>,
        )>,
    ) -> Result<FunctionPlanGeneration, FunctionPlanStoreError> {
        let basis = FunctionPlanBasis {
            registry_fingerprint,
            resource_versions,
        };
        let mut plans = BTreeMap::new();
        for (path, version, plan, abi) in entries {
            let source_facts = validate_plan(
                &self.project_session_id,
                &basis,
                &path,
                &version,
                plan.as_ref(),
            )?;
            validate_abi(&path, plan.as_ref(), abi.as_ref(), &source_facts)?;
            let key = FunctionPlanKey { path, version };
            let published = Arc::new(PublishedFunctionPlan { plan, abi });
            if plans.insert(key.clone(), published).is_some() {
                return Err(FunctionPlanStoreError::Duplicate { path: key.path });
            }
        }
        Ok(FunctionPlanGeneration {
            project_session_id: self.project_session_id.clone(),
            basis,
            plans,
            recursion_limit: self.recursion_limit,
        })
    }
}

#[derive(Debug, Clone)]
struct FunctionPlanBasis {
    registry_fingerprint: RegistryFingerprint,
    resource_versions: ResourceVersionSet,
}

#[derive(Debug, Clone)]
pub struct PublishedFunctionPlan {
    pub plan: Arc<ExecutionPlan>,
    pub abi: Arc<FunctionPlanAbi>,
}

pub struct FunctionPlanGeneration {
    project_session_id: ProjectSessionId,
    basis: FunctionPlanBasis,
    plans: BTreeMap<FunctionPlanKey, Arc<PublishedFunctionPlan>>,
    recursion_limit: usize,
}

impl FunctionPlanGeneration {
    pub fn plan_count(&self) -> usize {
        self.plans.len()
    }

    fn current_function(
        &self,
        path: &GraphResourcePath,
    ) -> Result<Option<Arc<PublishedFunctionPlan>>, FunctionPlanStoreError> {
        let key = ResourceKey::new(path.0.clone());
        let Some(version) = self.basis.resource_versions.get(&key) else {
            return Ok(None);
        };
        let plan_key = FunctionPlanKey {
            path: path.clone(),
            version: version.clone(),
        };
        let Some(function) = self.plans.get(&plan_key).cloned() else {
            return Ok(None);
        };
        let source_facts = validate_plan(
            &self.project_session_id,
            &self.basis,
            path,
            version,
            function.plan.as_ref(),
        )?;
        validate_abi(
            path,
            function.plan.as_ref(),
            function.abi.as_ref(),
            &source_facts,
        )?;
        Ok(Some(function))
    }
}

impl FunctionPlanProvider for FunctionPlanGeneration {
    fn get_function(
        &self,
        handle: &FunctionPlanHandle,
    ) -> Result<Option<Arc<PublishedFunctionPlan>>, Box<str>> {
        self.current_function(&GraphResourcePath(handle.as_str().into()))
            .map_err(|error| error.to_string().into())
    }

    fn recursion_limit(&self) -> usize {
        self.recursion_limit
    }
}

fn validate_abi(
    path: &GraphResourcePath,
    plan: &ExecutionPlan,
    abi: &FunctionPlanAbi,
    source_facts: &PlanSourceFacts,
) -> Result<(), FunctionPlanStoreError> {
    if abi.provenance != plan.provenance {
        return Err(FunctionPlanStoreError::InvalidBasis {
            path: path.clone(),
            message: "ABI provenance does not exactly match its plan".into(),
        });
    }
    if abi.results.keys().collect::<BTreeSet<_>>()
        != abi.result_productions.keys().collect::<BTreeSet<_>>()
    {
        return Err(FunctionPlanStoreError::InvalidBasis {
            path: path.clone(),
            message: "ABI results and result productions must have identical keys".into(),
        });
    }
    for (result, value) in &abi.results {
        let declared = abi.result_productions[result];
        let Some(actual) = source_facts.production(*value) else {
            return Err(FunctionPlanStoreError::InvalidBasis {
                path: path.clone(),
                message: format!(
                    "ABI result production for value {} cannot be reconstructed from its plan",
                    value.index()
                )
                .into(),
            });
        };
        if declared != actual {
            return Err(FunctionPlanStoreError::InvalidBasis {
                path: path.clone(),
                message: format!(
                    "ABI result production for value {} is {declared:?}, but its plan produces {actual:?}",
                    value.index()
                )
                .into(),
            });
        }
    }
    for (direction, members) in [("parameter", &abi.parameters), ("result", &abi.results)] {
        let mut values = BTreeSet::new();
        for value in members.values() {
            if value.index() >= plan.value_count as usize {
                return Err(FunctionPlanStoreError::InvalidBasis {
                    path: path.clone(),
                    message: format!("ABI value {} is outside the callee frame", value.index())
                        .into(),
                });
            }
            if !values.insert(*value) {
                return Err(FunctionPlanStoreError::InvalidBasis {
                    path: path.clone(),
                    message: format!(
                        "multiple ABI {direction} members alias value {}",
                        value.index()
                    )
                    .into(),
                });
            }
            let source_is_valid = match direction {
                "parameter" => source_facts.is_external_input(*value),
                "result" => source_facts.is_statically_sourced(*value),
                _ => unreachable!(),
            };
            if !source_is_valid {
                let requirement = if direction == "parameter" {
                    "declared ExternalInput"
                } else {
                    "statically producible"
                };
                return Err(FunctionPlanStoreError::InvalidBasis {
                    path: path.clone(),
                    message: format!(
                        "ABI {direction} value {} is not {requirement}",
                        value.index()
                    )
                    .into(),
                });
            }
        }
    }
    Ok(())
}

fn validate_plan(
    project_session_id: &ProjectSessionId,
    current: &FunctionPlanBasis,
    path: &GraphResourcePath,
    version: &ResourceVersion,
    plan: &ExecutionPlan,
) -> Result<PlanSourceFacts, FunctionPlanStoreError> {
    let source_facts = plan.validate_with_source_facts().map_err(|error| {
        FunctionPlanStoreError::InvalidBasis {
            path: path.clone(),
            message: format!("execution plan is invalid: {error}").into(),
        }
    })?;
    if &plan.provenance.project_session_id != project_session_id {
        return Err(FunctionPlanStoreError::InvalidBasis {
            path: path.clone(),
            message: "project session does not match the store".into(),
        });
    }
    if &plan.provenance.graph_path != path {
        return Err(FunctionPlanStoreError::InvalidBasis {
            path: path.clone(),
            message: "plan graph path does not match its index".into(),
        });
    }
    let resource_key = ResourceKey::new(path.0.clone());
    if current.resource_versions.get(&resource_key) != Some(version) {
        return Err(FunctionPlanStoreError::Stale {
            path: path.clone(),
            message: format!("resource version '{}' is not current", version.as_str()).into(),
        });
    }
    if plan.provenance.basis.registry_fingerprint != current.registry_fingerprint {
        return Err(FunctionPlanStoreError::InvalidBasis {
            path: path.clone(),
            message: "registry fingerprint does not match the current basis".into(),
        });
    }
    if !plan
        .provenance
        .basis
        .resource_versions
        .iter()
        .all(|(key, version)| current.resource_versions.get(key) == Some(version))
    {
        return Err(FunctionPlanStoreError::InvalidBasis {
            path: path.clone(),
            message: "resource versions do not match the current basis".into(),
        });
    }
    Ok(source_facts)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionPlanStoreError {
    Stale {
        path: GraphResourcePath,
        message: Box<str>,
    },
    InvalidBasis {
        path: GraphResourcePath,
        message: Box<str>,
    },
    Duplicate {
        path: GraphResourcePath,
    },
}

impl fmt::Display for FunctionPlanStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stale { path, message } => {
                write!(formatter, "function plan '{}' is stale: {message}", path.0)
            }
            Self::InvalidBasis { path, message } => write!(
                formatter,
                "function plan '{}' has invalid basis: {message}",
                path.0
            ),
            Self::Duplicate { path } => {
                write!(formatter, "function plan '{}' was published twice", path.0)
            }
        }
    }
}

impl std::error::Error for FunctionPlanStoreError {}
