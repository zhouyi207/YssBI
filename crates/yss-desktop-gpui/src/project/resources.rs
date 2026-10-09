//! Shared presentation of the workbench's accepted project index.
use std::collections::BTreeMap;
use yss_harness_contract::ResourceRevisionKind;
use yss_project::ProjectIndex;
use yss_project_identity::{
    ProjectInstanceId, ProjectResourceKind as Kind, ProjectResourceRef, ResourceRevision,
};

pub(crate) struct ResourceEntry {
    pub resource: ProjectResourceRef,
    pub name: String,
    revision: ResourceRevision,
    function_revision: Option<ResourceRevision>,
}

impl ResourceEntry {
    pub fn revision(&self, kind: ResourceRevisionKind) -> Option<u64> {
        match kind {
            ResourceRevisionKind::Resource => Some(self.revision.get()),
            ResourceRevisionKind::FunctionSignature => {
                self.function_revision.map(|revision| revision.get())
            }
        }
    }
}

pub(crate) struct ResourceCatalog {
    pub project: ProjectInstanceId,
    pub entries: Vec<ResourceEntry>,
    positions: BTreeMap<ProjectResourceRef, usize>,
    search: Vec<String>,
}

impl ResourceCatalog {
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
                item.chart_path.as_str(),
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

    pub fn matches(&self, query: &str) -> Vec<usize> {
        let query = query.trim().to_lowercase();
        self.search
            .iter()
            .enumerate()
            .filter_map(|(i, text)| text.contains(&query).then_some(i))
            .collect()
    }
}
