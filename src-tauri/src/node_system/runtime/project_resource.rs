use super::{PlotSink, ResourceError, ResourceLease, ResourceProvider};
use crate::graph::value::DataValue;
use crate::node_system::analysis::{
    CompileProvenance, ProjectSessionId, ResourceKey, ResourceVersion, ResourceVersionSet,
};
use crate::node_system::plan::{
    CompiledResourceRequirement, ResourceAccess, ResourceId, ResourceKind,
};
use crate::variable::VariableInstance;
use polars::prelude::DataFrame;
use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectResourceVersionFingerprint(Box<str>);

impl ProjectResourceVersionFingerprint {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone)]
pub struct ProjectResourceSnapshot {
    project_session_id: ProjectSessionId,
    versions: ResourceVersionSet,
    variables: BTreeMap<ResourceId, Arc<dyn ProjectVariableAccess>>,
    variable_effects: Arc<Mutex<BTreeMap<ResourceId, VariableWriteEffect>>>,
    databases: BTreeMap<ResourceId, ProjectDatabaseSnapshot>,
    plot_sink: Option<Arc<dyn PlotSink>>,
}

impl ProjectResourceSnapshot {
    pub fn new(project_session_id: ProjectSessionId, versions: ResourceVersionSet) -> Self {
        Self {
            project_session_id,
            versions,
            variables: BTreeMap::new(),
            variable_effects: Arc::new(Mutex::new(BTreeMap::new())),
            databases: BTreeMap::new(),
            plot_sink: None,
        }
    }

    pub fn with_variable(mut self, id: ResourceId, variable: Arc<VariableInstance>) -> Self {
        self = self.with_variable_revision(
            id,
            variable,
            crate::node_system::document::ResourceRevision::INITIAL,
        );
        self
    }

    pub fn with_variable_revision(
        mut self,
        id: ResourceId,
        variable: Arc<VariableInstance>,
        revision: crate::node_system::document::ResourceRevision,
    ) -> Self {
        self.variables.insert(
            id.clone(),
            Arc::new(SnapshotVariableAccess {
                resource: id,
                variable,
                revision,
                effects: Arc::clone(&self.variable_effects),
            }),
        );
        self
    }

    pub fn variable_effects(&self) -> Vec<VariableWriteEffect> {
        self.variable_effects
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .cloned()
            .collect()
    }

    pub fn with_variable_access(
        mut self,
        id: ResourceId,
        variable: Arc<dyn ProjectVariableAccess>,
    ) -> Self {
        self.variables.insert(id, variable);
        self
    }

    pub fn with_plot_sink(mut self, sink: Arc<dyn PlotSink>) -> Self {
        self.plot_sink = Some(sink);
        self
    }

    pub fn with_database(mut self, id: ResourceId, dataframe: Arc<DataFrame>) -> Self {
        self.databases
            .insert(id, ProjectDatabaseSnapshot::Loaded(dataframe));
        self
    }

    pub fn with_duckdb_database(
        mut self,
        id: ResourceId,
        path: impl Into<Box<str>>,
        table: impl Into<Box<str>>,
    ) -> Self {
        self.databases.insert(
            id,
            ProjectDatabaseSnapshot::DuckDb {
                path: path.into(),
                table: table.into(),
            },
        );
        self
    }

    pub fn project_session_id(&self) -> &ProjectSessionId {
        &self.project_session_id
    }

    pub fn versions(&self) -> &ResourceVersionSet {
        &self.versions
    }

    pub fn version_fingerprint(&self) -> ProjectResourceVersionFingerprint {
        let mut canonical = String::from("yssbi.project-resource-versions.v1|");
        for (key, version) in &self.versions {
            append_component(&mut canonical, key.as_str());
            append_component(&mut canonical, version.as_str());
        }
        ProjectResourceVersionFingerprint(canonical.into())
    }
}

fn append_component(output: &mut String, value: &str) {
    use std::fmt::Write;
    write!(output, "{}:", value.len()).expect("writing to a String cannot fail");
    output.push_str(value);
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct ProjectResourceLeaseObserver {
    counts: Arc<ProjectResourceLeaseCounts>,
}

#[cfg(test)]
#[derive(Default)]
struct ProjectResourceLeaseCounts {
    acquired: AtomicUsize,
    dropped: AtomicUsize,
    active: AtomicUsize,
}

#[cfg(test)]
impl ProjectResourceLeaseObserver {
    pub(crate) fn acquired(&self) -> usize {
        self.counts.acquired.load(Ordering::SeqCst)
    }

    pub(crate) fn dropped(&self) -> usize {
        self.counts.dropped.load(Ordering::SeqCst)
    }

    pub(crate) fn active(&self) -> usize {
        self.counts.active.load(Ordering::SeqCst)
    }

    fn observe_acquired(&self) {
        self.counts.acquired.fetch_add(1, Ordering::SeqCst);
        self.counts.active.fetch_add(1, Ordering::SeqCst);
    }

    fn observe_dropped(&self) {
        self.counts.dropped.fetch_add(1, Ordering::SeqCst);
        self.counts.active.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Clone)]
pub struct ProjectResourceProvider {
    snapshot: Arc<ProjectResourceSnapshot>,
    #[cfg(test)]
    lease_observer: Option<ProjectResourceLeaseObserver>,
}

impl ProjectResourceProvider {
    pub fn new(snapshot: ProjectResourceSnapshot) -> Self {
        Self {
            snapshot: Arc::new(snapshot),
            #[cfg(test)]
            lease_observer: None,
        }
    }

    pub fn from_shared(snapshot: Arc<ProjectResourceSnapshot>) -> Self {
        Self {
            snapshot,
            #[cfg(test)]
            lease_observer: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn set_lease_observer(&mut self, observer: ProjectResourceLeaseObserver) {
        self.lease_observer = Some(observer);
    }

    pub fn snapshot(&self) -> &ProjectResourceSnapshot {
        &self.snapshot
    }

    fn snapshot_contains(&self, requirement: &CompiledResourceRequirement) -> bool {
        match requirement.kind {
            ResourceKind::DatabaseConnection => {
                self.snapshot.databases.contains_key(&requirement.resource)
            }
            ResourceKind::ExternalArtifact => {
                self.snapshot.variables.contains_key(&requirement.resource)
                    || (requirement.resource.as_str() == super::kernels::PLOT_SINK
                        && self.snapshot.plot_sink.is_some())
            }
            ResourceKind::Accelerator | ResourceKind::Sidecar | ResourceKind::TemporaryStorage => {
                false
            }
        }
    }
}

impl ResourceProvider for ProjectResourceProvider {
    fn validate_plan(
        &self,
        provenance: &CompileProvenance,
        requirements: &[CompiledResourceRequirement],
    ) -> Result<(), ResourceError> {
        if provenance.project_session_id != self.snapshot.project_session_id {
            return Err(ResourceError::snapshot_mismatch(format!(
                "plan belongs to project session '{}', but resources belong to '{}'",
                provenance.project_session_id.as_str(),
                self.snapshot.project_session_id.as_str()
            )));
        }
        for requirement in requirements {
            if !self.snapshot_contains(requirement) {
                continue;
            }
            let key = ResourceKey::new(requirement.resource.as_str());
            let Some(expected) = provenance.basis.resource_versions.get(&key) else {
                return Err(ResourceError::snapshot_mismatch(format!(
                    "plan has no version for project resource '{}'",
                    requirement.resource.as_str()
                )));
            };
            let Some(actual) = self.snapshot.versions.get(&key) else {
                return Err(ResourceError::snapshot_mismatch(format!(
                    "snapshot has no version for project resource '{}'",
                    requirement.resource.as_str()
                )));
            };
            if actual != expected {
                return Err(stale_version(&requirement.resource, expected, actual));
            }
        }
        Ok(())
    }

    fn acquire(
        &self,
        requirement: &CompiledResourceRequirement,
    ) -> Result<Box<dyn ResourceLease>, ResourceError> {
        if requirement.access == ResourceAccess::Exclusive
            && !self.snapshot.variables.contains_key(&requirement.resource)
        {
            return Err(ResourceError::new(format!(
                "project resource '{}' does not support exclusive access",
                requirement.resource.as_str()
            )));
        }
        let value = match requirement.kind {
            ResourceKind::DatabaseConnection => self
                .snapshot
                .databases
                .get(&requirement.resource)
                .cloned()
                .map(ProjectResourceValue::Database),
            ResourceKind::ExternalArtifact
                if requirement.resource.as_str() == super::kernels::PLOT_SINK =>
            {
                self.snapshot
                    .plot_sink
                    .clone()
                    .map(ProjectResourceValue::PlotSink)
            }
            ResourceKind::ExternalArtifact => self
                .snapshot
                .variables
                .get(&requirement.resource)
                .cloned()
                .map(ProjectResourceValue::Variable),
            ResourceKind::Accelerator | ResourceKind::Sidecar | ResourceKind::TemporaryStorage => {
                None
            }
        }
        .ok_or_else(|| {
            ResourceError::new(format!(
                "project resource '{}' is unavailable for {:?}",
                requirement.resource.as_str(),
                requirement.kind
            ))
        })?;
        #[cfg(test)]
        if let Some(observer) = &self.lease_observer {
            observer.observe_acquired();
        }
        Ok(Box::new(ProjectResourceLease {
            resource: requirement.resource.clone(),
            value,
            #[cfg(test)]
            lease_observer: self.lease_observer.clone(),
        }))
    }
}

fn stale_version(
    resource: &ResourceId,
    expected: &ResourceVersion,
    actual: &ResourceVersion,
) -> ResourceError {
    ResourceError::snapshot_mismatch(format!(
        "project resource '{}' is stale: plan requires '{}', snapshot has '{}'",
        resource.as_str(),
        expected.as_str(),
        actual.as_str()
    ))
}

pub trait ProjectVariableAccess: Send + Sync {
    fn read(&self) -> Result<VariableInstance, Box<str>>;
    fn write(&self, value: DataValue) -> Result<VariableInstance, Box<str>>;
}

#[derive(Debug, Clone)]
pub struct VariableWriteEffect {
    pub resource: ResourceId,
    pub expected_revision: crate::node_system::document::ResourceRevision,
    pub before: VariableInstance,
    pub after: DataValue,
}

struct SnapshotVariableAccess {
    resource: ResourceId,
    variable: Arc<VariableInstance>,
    revision: crate::node_system::document::ResourceRevision,
    effects: Arc<Mutex<BTreeMap<ResourceId, VariableWriteEffect>>>,
}

impl ProjectVariableAccess for SnapshotVariableAccess {
    fn read(&self) -> Result<VariableInstance, Box<str>> {
        Ok(self.variable.as_ref().clone())
    }

    fn write(&self, value: DataValue) -> Result<VariableInstance, Box<str>> {
        let mut updated = self.variable.as_ref().clone();
        updated.data_value = value.clone();
        self.effects
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                self.resource.clone(),
                VariableWriteEffect {
                    resource: self.resource.clone(),
                    expected_revision: self.revision,
                    before: self.variable.as_ref().clone(),
                    after: value,
                },
            );
        Ok(updated)
    }
}

#[derive(Debug, Clone)]
pub struct ProjectDataFrameScan {
    pub dataframe: Arc<DataFrame>,
    pub applied_limit: Option<usize>,
}

#[derive(Clone)]
pub enum ProjectDatabaseSnapshot {
    Loaded(Arc<DataFrame>),
    DuckDb { path: Box<str>, table: Box<str> },
}

impl ProjectDatabaseSnapshot {
    pub fn load(&self) -> Result<Arc<DataFrame>, Box<str>> {
        self.load_bounded(None).map(|scan| scan.dataframe)
    }

    pub fn load_bounded(&self, limit: Option<usize>) -> Result<ProjectDataFrameScan, Box<str>> {
        match self {
            Self::Loaded(dataframe) => {
                let dataframe = match limit {
                    Some(limit) => Arc::new(dataframe.head(Some(limit))),
                    None => Arc::clone(dataframe),
                };
                Ok(ProjectDataFrameScan {
                    dataframe,
                    applied_limit: limit,
                })
            }
            Self::DuckDb { path, table } => {
                let table_sql = crate::database::duckdb_table_sql(table);
                let sql = match limit {
                    Some(limit) => format!("SELECT * FROM {table_sql} LIMIT {limit}"),
                    None => format!("SELECT * FROM {table_sql}"),
                };
                crate::database::query_to_dataframe_for_table(
                    std::path::Path::new(path.as_ref()),
                    &sql,
                    Some(table.as_ref()),
                )
                .map(|dataframe| ProjectDataFrameScan {
                    dataframe: Arc::new(dataframe),
                    applied_limit: limit,
                })
                .map_err(Into::into)
            }
        }
    }
}

pub enum ProjectResourceValue {
    Variable(Arc<dyn ProjectVariableAccess>),
    Database(ProjectDatabaseSnapshot),
    PlotSink(Arc<dyn PlotSink>),
}

pub struct ProjectResourceLease {
    resource: ResourceId,
    value: ProjectResourceValue,
    #[cfg(test)]
    lease_observer: Option<ProjectResourceLeaseObserver>,
}

impl ProjectResourceLease {
    pub fn variable_access(&self) -> Option<&dyn ProjectVariableAccess> {
        match &self.value {
            ProjectResourceValue::Variable(variable) => Some(variable.as_ref()),
            ProjectResourceValue::Database(_) | ProjectResourceValue::PlotSink(_) => None,
        }
    }

    pub fn plot_sink(&self) -> Option<&dyn PlotSink> {
        match &self.value {
            ProjectResourceValue::PlotSink(sink) => Some(sink.as_ref()),
            ProjectResourceValue::Variable(_) | ProjectResourceValue::Database(_) => None,
        }
    }

    pub fn dataframe(&self) -> Option<&DataFrame> {
        match &self.value {
            ProjectResourceValue::Database(ProjectDatabaseSnapshot::Loaded(dataframe)) => {
                Some(dataframe)
            }
            ProjectResourceValue::Database(ProjectDatabaseSnapshot::DuckDb { .. })
            | ProjectResourceValue::Variable(_)
            | ProjectResourceValue::PlotSink(_) => None,
        }
    }

    pub fn load_dataframe(&self) -> Result<Option<Arc<DataFrame>>, Box<str>> {
        match &self.value {
            ProjectResourceValue::Database(database) => database.load().map(Some),
            ProjectResourceValue::Variable(_) | ProjectResourceValue::PlotSink(_) => Ok(None),
        }
    }

    pub fn scan_dataframe(
        &self,
        limit: Option<usize>,
    ) -> Result<Option<ProjectDataFrameScan>, Box<str>> {
        match &self.value {
            ProjectResourceValue::Database(database) => database.load_bounded(limit).map(Some),
            ProjectResourceValue::Variable(_) | ProjectResourceValue::PlotSink(_) => Ok(None),
        }
    }
}

#[cfg(test)]
impl Drop for ProjectResourceLease {
    fn drop(&mut self) {
        if let Some(observer) = &self.lease_observer {
            observer.observe_dropped();
        }
    }
}

impl ResourceLease for ProjectResourceLease {
    fn resource_id(&self) -> &ResourceId {
        &self.resource
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl fmt::Debug for ProjectResourceLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectResourceLease")
            .field("resource", &self.resource)
            .finish_non_exhaustive()
    }
}
