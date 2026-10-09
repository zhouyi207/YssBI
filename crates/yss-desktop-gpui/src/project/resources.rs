//! Shared presentation of the workbench's accepted project index.
use std::collections::BTreeMap;
use yss_harness_contract::HarnessResourceReference;
use yss_project::ProjectIndex;
use yss_project_identity::{ProjectInstanceId, ProjectResourceKind as Kind, ProjectResourceRef};

pub(crate) struct ResourceCatalog {
    pub project: ProjectInstanceId,
    pub entries: Vec<HarnessResourceReference>,
    positions: BTreeMap<ProjectResourceRef, usize>,
    search: Vec<String>,
}

impl ResourceCatalog {
    pub fn new(project: ProjectInstanceId, index: &ProjectIndex) -> Self {
        let mut entries = vec![];
        let mut add = |kind, id: &str, name: &str| {
            entries.push(HarnessResourceReference {
                resource: ProjectResourceRef {
                    kind,
                    id: id.to_owned(),
                },
                name: name.to_owned(),
            });
        };
        for item in &index.databases {
            add(
                Kind::Database,
                &item.id,
                item.name.as_deref().unwrap_or(&item.id),
            );
        }
        for item in &index.event_graphs {
            add(Kind::EventGraph, &item.path, &item.name);
        }
        for item in &index.function_graphs {
            add(Kind::FunctionGraph, &item.path, &item.name);
        }
        for item in &index.charts {
            add(Kind::Chart, item.chart_path.as_str(), &item.name);
        }
        for item in &index.docs {
            add(Kind::Doc, item.path.as_str(), &item.name);
        }
        for item in &index.minds {
            add(Kind::Mind, item.path.as_str(), &item.name);
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

    pub fn get(&self, resource: &ProjectResourceRef) -> Option<&HarnessResourceReference> {
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
