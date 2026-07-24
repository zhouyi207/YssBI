use super::FunctionPlanProvider;
use crate::node_system::analysis::{
    ProjectSessionId, ResourceKey, ResourceVersion, ResourceVersionSet,
};
use crate::node_system::document::GraphResourcePath;
use crate::node_system::plan::{ExecutionPlan, FunctionPlanHandle};
use crate::node_system::registry::RegistryFingerprint;
use std::collections::BTreeMap;
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
        entries: Vec<(GraphResourcePath, ResourceVersion, Arc<ExecutionPlan>)>,
    ) -> Result<FunctionPlanGeneration, FunctionPlanStoreError> {
        let basis = FunctionPlanBasis {
            registry_fingerprint,
            resource_versions,
        };
        let mut plans = BTreeMap::new();
        for (path, version, plan) in entries {
            validate_plan(
                &self.project_session_id,
                &basis,
                &path,
                &version,
                plan.as_ref(),
            )?;
            let key = FunctionPlanKey { path, version };
            if plans.insert(key.clone(), plan).is_some() {
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

pub struct FunctionPlanGeneration {
    project_session_id: ProjectSessionId,
    basis: FunctionPlanBasis,
    plans: BTreeMap<FunctionPlanKey, Arc<ExecutionPlan>>,
    recursion_limit: usize,
}

impl FunctionPlanGeneration {
    pub fn plan_count(&self) -> usize {
        self.plans.len()
    }

    fn current_plan(
        &self,
        path: &GraphResourcePath,
    ) -> Result<Option<Arc<ExecutionPlan>>, FunctionPlanStoreError> {
        let key = ResourceKey::new(path.0.clone());
        let Some(version) = self.basis.resource_versions.get(&key) else {
            return Ok(None);
        };
        let plan_key = FunctionPlanKey {
            path: path.clone(),
            version: version.clone(),
        };
        let Some(plan) = self.plans.get(&plan_key).cloned() else {
            return Ok(None);
        };
        validate_plan(
            &self.project_session_id,
            &self.basis,
            path,
            version,
            plan.as_ref(),
        )?;
        Ok(Some(plan))
    }
}

impl FunctionPlanProvider for FunctionPlanGeneration {
    fn get_plan(
        &self,
        handle: &FunctionPlanHandle,
    ) -> Result<Option<Arc<ExecutionPlan>>, Box<str>> {
        self.current_plan(&GraphResourcePath(handle.as_str().into()))
            .map_err(|error| error.to_string().into())
    }

    fn recursion_limit(&self) -> usize {
        self.recursion_limit
    }
}

fn validate_plan(
    project_session_id: &ProjectSessionId,
    current: &FunctionPlanBasis,
    path: &GraphResourcePath,
    version: &ResourceVersion,
    plan: &ExecutionPlan,
) -> Result<(), FunctionPlanStoreError> {
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
    if plan.provenance.basis.resource_versions != current.resource_versions {
        return Err(FunctionPlanStoreError::InvalidBasis {
            path: path.clone(),
            message: "resource versions do not match the current basis".into(),
        });
    }
    Ok(())
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
