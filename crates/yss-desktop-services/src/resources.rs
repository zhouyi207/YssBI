//! Shared presentation of the workbench's accepted project index.
use std::collections::BTreeMap;
use yss_assistant_contract::ResourceRevisionKind;
use yss_project::ProjectIndex;
use yss_project_identity::{
    ProjectInstanceId, ProjectResourceKind as Kind, ProjectResourceRef, ResourceRevision,
};

#[derive(Clone)]
pub struct ResourceEntry {
    resource: ProjectResourceRef,
    name: String,
    revision: ResourceRevision,
    function_revision: Option<ResourceRevision>,
}

impl ResourceEntry {
    pub fn resource(&self) -> &ProjectResourceRef {
        &self.resource
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn revision(&self, kind: ResourceRevisionKind) -> Option<u64> {
        match kind {
            ResourceRevisionKind::Resource => Some(self.revision.get()),
            ResourceRevisionKind::FunctionSignature => {
                self.function_revision.map(|revision| revision.get())
            }
        }
    }
}

#[derive(Clone)]
pub struct ResourceCatalog {
    project: ProjectInstanceId,
    entries: Vec<ResourceEntry>,
    positions: BTreeMap<ProjectResourceRef, usize>,
    search: Vec<String>,
}

impl ResourceCatalog {
    pub fn project(&self) -> &ProjectInstanceId {
        &self.project
    }

    pub fn entries(&self) -> &[ResourceEntry] {
        &self.entries
    }

    pub fn new(project: ProjectInstanceId, index: &ProjectIndex) -> Self {
        let mut entries = vec![];
        let mut add = |kind, id: &str, name: &str, revision, function_revision| {
            entries.push(ResourceEntry {
                resource: ProjectResourceRef {
                    kind,
                    id: id.to_owned(),
                },
                name: name.to_owned(),
                revision,
                function_revision,
            });
        };
        for item in &index.databases {
            add(
                Kind::Database,
                &item.id,
                item.name.as_deref().unwrap_or(&item.id),
                item.revision,
                None,
            );
        }
        for item in &index.event_graphs {
            add(
                Kind::EventGraph,
                &item.path,
                &item.name,
                item.revision,
                None,
            );
        }
        for item in &index.function_graphs {
            add(
                Kind::FunctionGraph,
                &item.path,
                &item.name,
                item.revision,
                Some(item.function_revision),
            );
        }
        for item in &index.charts {
            add(
                Kind::Chart,
                item.path.as_str(),
                &item.name,
                item.revision,
                None,
            );
        }
        for item in &index.docs {
            add(
                Kind::Doc,
                item.path.as_str(),
                &item.name,
                item.revision,
                None,
            );
        }
        for item in &index.minds {
            add(
                Kind::Mind,
                item.path.as_str(),
                &item.name,
                item.revision,
                None,
            );
        }
        entries.sort_by_cached_key(|entry| (entry.name.to_lowercase(), entry.resource.clone()));
        let positions = entries
            .iter()
            .enumerate()
            .map(|(i, entry)| (entry.resource.clone(), i))
            .collect();
        let search = entries
            .iter()
            .map(|entry| format!("{} {}", entry.name, entry.resource.id).to_lowercase())
            .collect();
        Self {
            project,
            entries,
            positions,
            search,
        }
    }

    pub fn get(&self, resource: &ProjectResourceRef) -> Option<&ResourceEntry> {
        self.entries.get(*self.positions.get(resource)?)
    }

    /// Advances a retained catalog entry without rebuilding names, order or search rows.
    pub fn set_resource_revision(&mut self, kind: Kind, id: &str, revision: ResourceRevision) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.resource.kind == kind && entry.resource.id == id)
        {
            entry.revision = revision;
        }
    }

    pub fn matches(&self, query: &str) -> Vec<usize> {
        let query = query.trim().to_lowercase();
        self.search
            .iter()
            .enumerate()
            .filter_map(|(i, text)| text.contains(&query).then_some(i))
            .collect()
    }
}
