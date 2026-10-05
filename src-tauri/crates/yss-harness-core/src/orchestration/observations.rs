//! Read baselines projected from receipts, independent of model context compaction.

use std::collections::{BTreeMap, BTreeSet};
use yss_harness_contract::*;

#[derive(Clone)]
enum Observation {
    Read(ResourceVersion),
    ChangedByWorker,
    PreviousSession,
}

#[derive(Clone)]
pub(super) struct GraphBasis {
    pub version: ResourceVersion,
    pub hash: String,
    pub inputs: String,
}

#[derive(Default)]
pub(crate) struct ResourceObservations {
    resources: BTreeMap<ProjectResourceRef, Observation>,
    graphs: BTreeMap<String, Observation>,
    datasets: BTreeMap<String, (u64, u64)>,
    graph_details: BTreeMap<String, GraphBasis>,
    signatures: BTreeMap<String, (ResourceVersion, u64)>,
}

impl ResourceObservations {
    pub(super) fn graph_inputs(&self, path: &str) -> Option<String> {
        self.graph_details
            .get(path)
            .map(|basis| basis.inputs.clone())
    }

    pub(super) fn refresh_from(&mut self, source: &Self, resource: &ProjectResourceRef) {
        if let Some(basis) = source.graph_details.get(&resource.id) {
            self.graph_details
                .insert(resource.id.clone(), basis.clone());
        }
        if let Some(signature) = source.signatures.get(&resource.id) {
            self.signatures
                .insert(resource.id.clone(), signature.clone());
        }
    }
    pub(super) fn graph_basis(&self, path: &str, version: &ResourceVersion) -> Option<GraphBasis> {
        self.graph_details
            .get(path)
            .filter(|basis| &basis.version == version)
            .cloned()
    }

    pub(super) fn signature_revision(&self, path: &str, version: &ResourceVersion) -> Option<u64> {
        self.signatures
            .get(path)
            .filter(|(basis, _)| basis == version)
            .map(|(_, revision)| *revision)
    }
    fn observation(&self, resource: &ProjectResourceRef) -> Option<&Observation> {
        self.resources.get(resource).or_else(|| {
            matches!(
                resource.kind,
                ProjectResourceKind::EventGraph | ProjectResourceKind::FunctionGraph
            )
            .then(|| self.graphs.get(&resource.id))
            .flatten()
        })
    }

    pub(super) fn version(
        &self,
        resource: &ProjectResourceRef,
        resume: bool,
    ) -> Result<Option<ResourceVersion>, CapabilityFailure> {
        match self.observation(resource) {
            Some(Observation::Read(version)) => Ok(Some(version.clone())),
            // A resumed worker can use its own committed receipts within the same project session.
            Some(Observation::ChangedByWorker) if resume => Ok(None),
            Some(Observation::ChangedByWorker | Observation::PreviousSession) => Err(
                CapabilityFailure::new(CapabilityFailureCode::RevisionConflict)
                    .with_detail("reason", "resource_requires_current_read")
                    .with_detail("resourceId", &resource.id)
                    .with_detail("nextStep", "Inspect the resource again and reassess the task before delegating or resuming.")),
            None => Ok(None),
        }
    }

    pub(crate) fn replay(
        &mut self,
        record: &ToolInvocationRecord,
        project: &ProjectSessionBinding,
    ) {
        if let Some(result) = &record.result {
            self.record_from_session(result, &record.project == project);
        }
    }

    pub(crate) fn record(&mut self, result: &AutomationCapabilityResult) {
        self.record_from_session(result, true);
    }

    fn record_from_session(&mut self, result: &AutomationCapabilityResult, current: bool) {
        let observed = |version: &ResourceVersion| {
            if current {
                Observation::Read(version.clone())
            } else {
                Observation::PreviousSession
            }
        };
        match result {
            AutomationCapabilityResult::ResourceInspection(value) => {
                if current && let ResourceContent::Database { schema, .. } = &value.content {
                    self.datasets.insert(
                        schema.database_id.clone(),
                        (schema.runtime_revision, schema.schema_revision),
                    );
                }
                self.resources
                    .insert(value.resource.clone(), observed(&value.version));
                let (graph, function) = match &value.content {
                    ResourceContent::Graph {
                        graph, function, ..
                    } => (
                        Some((&graph.graph_hash, &graph.semantic_input_hash)),
                        function.as_ref(),
                    ),
                    ResourceContent::GraphPage {
                        graph, function, ..
                    } => (
                        Some((&graph.graph_hash, &graph.semantic_input_hash)),
                        function.as_ref(),
                    ),
                    _ => (None, None),
                };
                if let Some((hash, inputs)) = graph {
                    self.graph_detail(&value.resource.id, &value.version, hash, inputs, current);
                }
                if current && let Some(function) = function {
                    self.signatures.insert(
                        value.resource.id.clone(),
                        (value.version.clone(), function.revision),
                    );
                } else if !current {
                    self.signatures.remove(&value.resource.id);
                }
            }
            AutomationCapabilityResult::GraphInspection(value) => {
                self.graph(&value.graph_path, observed(&value.version));
                self.graph_detail(
                    &value.graph_path,
                    &value.version,
                    &value.graph_hash,
                    &value.semantic_input_hash,
                    current,
                );
            }
            AutomationCapabilityResult::GraphInspectionPage(value) => {
                self.graph(&value.graph_path, observed(&value.version));
                self.graph_detail(
                    &value.graph_path,
                    &value.version,
                    &value.graph_hash,
                    &value.semantic_input_hash,
                    current,
                );
            }
            AutomationCapabilityResult::DatasetSchemaInspection(value) => self.database(
                &value.database_id,
                value.runtime_revision,
                value.schema_revision,
                current,
            ),
            AutomationCapabilityResult::DatasetProfileInspection(value) => self.database(
                &value.database_id,
                value.runtime_revision,
                value.schema_revision,
                current,
            ),
            AutomationCapabilityResult::GraphEditReceipt(value) if current => self.advance_graph(
                &value.graph_path,
                value.from_revision,
                value.to_revision,
                &value.graph_hash,
                Some(&value.changes.semantic_input_hash),
            ),
            AutomationCapabilityResult::GraphSaved(value) if current => self.advance_graph(
                &value.graph_path,
                value.from_revision,
                value.resource_revision,
                &value.graph_hash,
                None,
            ),
            AutomationCapabilityResult::GraphExecution(value) if current => {
                // A run can discover dynamic columns without changing the graph document.
                self.graph_details.remove(&value.graph_path);
            }
            AutomationCapabilityResult::ResourceEdited(value)
            | AutomationCapabilityResult::ResourceManaged(value)
                if current =>
            {
                for change in &value.changes {
                    self.graph_details.remove(&change.resource.id);
                    if change.deleted {
                        self.signatures.remove(&change.resource.id);
                        self.resources
                            .insert(change.resource.clone(), Observation::ChangedByWorker);
                    } else if change.revision_kind == ResourceRevisionKind::Resource {
                        if let Some(Observation::Read(version)) =
                            self.resources.get_mut(&change.resource)
                        {
                            version.revision = change.revision;
                        }
                        if let Some((version, _)) = self.signatures.get_mut(&change.resource.id) {
                            version.revision = change.revision;
                        }
                    } else if let Some((_, revision)) = self.signatures.get_mut(&change.resource.id)
                    {
                        *revision = change.revision;
                    }
                }
                for moved in &value.moves {
                    self.graph_details.remove(&moved.from.id);
                    self.signatures.remove(&moved.from.id);
                }
            }
            _ => {}
        }
    }

    fn graph_detail(
        &mut self,
        path: &str,
        version: &ResourceVersion,
        hash: &str,
        inputs: &str,
        current: bool,
    ) {
        if current {
            self.graph_details.insert(
                path.into(),
                GraphBasis {
                    version: version.clone(),
                    hash: hash.into(),
                    inputs: inputs.into(),
                },
            );
        } else {
            self.graph_details.remove(path);
        }
    }

    fn advance_graph(&mut self, path: &str, from: u64, to: u64, hash: &str, inputs: Option<&str>) {
        let Some(basis) = self.graph_details.get_mut(path) else {
            return;
        };
        if basis.version.revision != from {
            return;
        }
        basis.version.revision = to;
        basis.hash = hash.into();
        if let Some(inputs) = inputs {
            basis.inputs = inputs.into();
        }
        let version = basis.version.clone();
        self.graph(path, Observation::Read(version));
        if let Some((version, _)) = self.signatures.get_mut(path)
            && version.revision == from
        {
            version.revision = to;
        }
    }

    fn graph(&mut self, path: &str, observation: Observation) {
        self.graphs.insert(path.into(), observation.clone());
        for (resource, observed) in &mut self.resources {
            if resource.id == path
                && matches!(
                    resource.kind,
                    ProjectResourceKind::EventGraph | ProjectResourceKind::FunctionGraph
                )
            {
                *observed = observation.clone();
            }
        }
    }

    fn database(&mut self, id: &str, runtime: u64, schema: u64, current: bool) {
        let resource = ProjectResourceRef {
            kind: ProjectResourceKind::Database,
            id: id.into(),
        };
        if current {
            self.datasets.insert(id.into(), (runtime, schema));
            // Runtime/schema counters belong to Database, not Project resource history.
            self.resources.remove(&resource);
        } else {
            self.datasets.remove(id);
            self.resources
                .insert(resource, Observation::PreviousSession);
        }
    }

    pub(super) fn needs_database_content(&self, resource: &ProjectResourceRef) -> bool {
        resource.kind == ProjectResourceKind::Database && self.datasets.contains_key(&resource.id)
    }

    pub(super) fn bind(
        &mut self,
        value: &ResourceInspection,
    ) -> Result<ResourceVersion, CapabilityFailure> {
        if self.needs_database_content(&value.resource) {
            let ResourceContent::Database { schema, .. } = &value.content else {
                return Err(CapabilityFailure::new(
                    CapabilityFailureCode::InternalFailure,
                ));
            };
            if self.datasets.get(&value.resource.id)
                != Some(&(schema.runtime_revision, schema.schema_revision))
            {
                return Err(
                    CapabilityFailure::new(CapabilityFailureCode::RevisionConflict)
                        .with_detail("reason", "dataset_changed_since_read")
                        .with_detail(
                            "nextStep",
                            "Inspect the current dataset and reassess the task before delegating.",
                        ),
                );
            }
        }
        self.resources.insert(
            value.resource.clone(),
            Observation::Read(value.version.clone()),
        );
        Ok(value.version.clone())
    }

    pub(crate) fn invalidate(&mut self, changes: &[ResourceChange]) {
        let mut seen = BTreeSet::new();
        for change in changes.iter().rev() {
            if !seen.insert((
                &change.resource,
                matches!(
                    change.revision_kind,
                    ResourceRevisionKind::FunctionSignature
                ),
            )) {
                continue;
            }
            if matches!(
                self.observation(&change.resource),
                Some(Observation::PreviousSession)
            ) {
                continue;
            }
            if !change.deleted
                && change.revision_kind == ResourceRevisionKind::Resource
                && self
                    .version(&change.resource, false)
                    .ok()
                    .flatten()
                    .is_some_and(|version| version.revision == change.revision)
            {
                continue;
            }
            // A worker receipt cannot silently replace the Manager's previously read basis.
            if self.observation(&change.resource).is_some()
                || self.needs_database_content(&change.resource)
            {
                self.resources
                    .insert(change.resource.clone(), Observation::ChangedByWorker);
                self.graphs.remove(&change.resource.id);
                self.graph_details.remove(&change.resource.id);
                self.signatures.remove(&change.resource.id);
            }
        }
    }
}
