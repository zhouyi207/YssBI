//! Read-only Activity panel projections. UI expansion and draft history stay in the client.
use std::collections::{BTreeMap, BTreeSet};

use yss_graph_catalog::{LocalizedCatalog, NodeCreation};
use yss_plugin_protocol::InstalledPlugin;
use yss_project_identity::ProjectInstanceId;

use crate::catalog_query::{CatalogQueryApplicationError, LocalizedCatalogRequest};
use crate::execution::ApplicationState;

#[derive(Debug)]
pub enum ActivityText {
    Key(&'static str),
    Literal(String),
}

#[derive(Debug)]
pub struct ActivityTool {
    pub id: &'static str,
    pub label: ActivityText,
    pub icon: &'static str,
}

#[derive(Debug)]
pub enum ActivityItem {
    Graph {
        path: String,
        name: String,
        graph_type: yss_graph_document::GraphResourceKind,
    },
    Chart {
        path: String,
        name: String,
    },
    Database {
        id: String,
        name: String,
        resource_path: String,
    },
    Node {
        key: String,
        title: String,
        creation: NodeCreation,
    },
    Command {
        id: &'static str,
        label: ActivityText,
    },
    Plugin {
        id: String,
        name: String,
        description: String,
        publisher: String,
        enabled: bool,
    },
}

#[derive(Debug)]
pub enum ActivityRowContent {
    Category {
        label: ActivityText,
        default_expanded: bool,
        tools: Vec<ActivityTool>,
        count: Option<usize>,
    },
    Item(ActivityItem),
    Message {
        label: ActivityText,
        description: Option<ActivityText>,
    },
}

#[derive(Debug)]
pub struct ActivityRow {
    pub id: String,
    pub depth: usize,
    pub content: ActivityRowContent,
}

#[derive(Debug)]
pub struct ActivityPanelDocument {
    pub panel_id: &'static str,
    pub project_instance_id: Option<String>,
    pub publication_revision: u64,
    pub title: ActivityText,
    pub tools: Vec<ActivityTool>,
    pub rows: Vec<ActivityRow>,
    pub empty_state: Option<(ActivityText, ActivityText)>,
}

impl ActivityPanelDocument {
    fn new(panel_id: &'static str, title: &'static str) -> Self {
        Self {
            panel_id,
            project_instance_id: None,
            publication_revision: 0,
            title: ActivityText::Key(title),
            tools: vec![],
            rows: vec![],
            empty_state: None,
        }
    }
}

/// Presentation only: the caller owns the coherent resource read.
pub fn project_activity_panel(index: Option<&yss_project::ProjectIndex>) -> ActivityPanelDocument {
    let mut document = ActivityPanelDocument::new("project", "activityBar.project");
    if let Some(index) = index {
        document.project_instance_id = Some(index.project_instance_id.clone());
        document.publication_revision = index.publication_revision;
    }
    for (kind, id, label, expanded, action, action_label, empty) in [
        (
            "event",
            "project.events",
            "sidebar.projectTree.categories.events",
            true,
            "newEvent",
            "canvas.newEventGraph",
            "sidebar.noEvents",
        ),
        (
            "function",
            "project.functions",
            "sidebar.projectTree.categories.functions",
            false,
            "newFunction",
            "canvas.newFunctionGraph",
            "sidebar.noFunctions",
        ),
        (
            "chart",
            "project.charts",
            "sidebar.projectTree.categories.charts",
            true,
            "newChart",
            "contextMenu.sidebar.newChart",
            "chartsSidebar.noCharts",
        ),
        (
            "database",
            "project.data",
            "sidebar.sections.data",
            true,
            "importData",
            "contextMenu.sidebar.importData",
            "sidebar.noData",
        ),
    ] {
        document.rows.push(ActivityRow {
            id: id.into(),
            depth: 0,
            content: ActivityRowContent::Category {
                label: ActivityText::Key(label),
                default_expanded: expanded,
                count: None,
                tools: vec![ActivityTool {
                    id: action,
                    label: ActivityText::Key(action_label),
                    icon: "add",
                }],
            },
        });
        let start = document.rows.len();
        if let Some(index) = index {
            match kind {
                "event" | "function" => {
                    let graph_kind = if kind == "event" {
                        yss_graph_document::GraphResourceKind::Event
                    } else {
                        yss_graph_document::GraphResourceKind::Function
                    };
                    document.rows.extend(
                        index
                            .graphs
                            .iter()
                            .filter(|graph| graph.graph_type == graph_kind)
                            .map(|graph| ActivityRow {
                                id: format!("{kind}:{}", graph.path),
                                depth: 1,
                                content: ActivityRowContent::Item(ActivityItem::Graph {
                                    path: graph.path.clone(),
                                    name: graph.name.clone(),
                                    graph_type: graph.graph_type,
                                }),
                            }),
                    );
                }
                "chart" => document
                    .rows
                    .extend(index.charts.iter().map(|chart| ActivityRow {
                        id: format!("chart:{}", chart.chart_path.as_str()),
                        depth: 1,
                        content: ActivityRowContent::Item(ActivityItem::Chart {
                            path: chart.chart_path.as_str().into(),
                            name: chart.name.clone(),
                        }),
                    })),
                _ => document
                    .rows
                    .extend(index.databases.iter().map(|database| ActivityRow {
                        id: format!("database:{}", database.id),
                        depth: 1,
                        content: ActivityRowContent::Item(ActivityItem::Database {
                            id: database.id.clone(),
                            name: database.name.clone().unwrap_or_else(|| database.id.clone()),
                            resource_path: database.resource_path.as_str().into(),
                        }),
                    })),
            }
        }
        if document.rows.len() == start {
            document
                .rows
                .push(message(&format!("{id}.empty"), 1, empty));
        }
    }
    document
}

pub(crate) fn nodes_activity_panel_from_catalog(
    result: crate::catalog_query::CatalogQueryResult,
) -> ActivityPanelDocument {
    let (project, _, revision, catalog) = result.into_transport_parts().into_fields();
    let mut document = ActivityPanelDocument::new("nodes", "activityBar.nodes");
    document.project_instance_id = Some(project.as_str().into());
    document.publication_revision = revision;
    document.rows = catalog_rows(catalog);
    if document.rows.is_empty() {
        document
            .rows
            .push(message("nodes.empty", 0, "sidebar.noNodes"));
    }
    document
}

impl ApplicationState {
    pub fn nodes_activity_panel(
        &self,
        project: Option<ProjectInstanceId>,
        locale: String,
    ) -> Result<ActivityPanelDocument, CatalogQueryApplicationError> {
        if let Some(project) = project {
            return self
                .localized_node_catalog(LocalizedCatalogRequest::new(project, locale))
                .map(nodes_activity_panel_from_catalog);
        }
        let mut document = ActivityPanelDocument::new("nodes", "activityBar.nodes");
        document
            .rows
            .push(message("nodes.empty", 0, "sidebar.noNodes"));
        Ok(document)
    }
}

fn message(id: &str, depth: usize, label: &'static str) -> ActivityRow {
    ActivityRow {
        id: id.into(),
        depth,
        content: ActivityRowContent::Message {
            label: ActivityText::Key(label),
            description: None,
        },
    }
}

fn catalog_rows(catalog: LocalizedCatalog) -> Vec<ActivityRow> {
    let categories = catalog.categories;
    let categories: BTreeMap<_, _> = categories
        .iter()
        .map(|category| (category.category_id.as_ref(), category))
        .collect();
    let mut items: BTreeMap<&str, Vec<_>> = BTreeMap::new();
    for item in &catalog.items {
        items.entry(&item.category_id).or_default().push(item);
    }
    for group in items.values_mut() {
        group.sort_by(|a, b| {
            a.title
                .cmp(&b.title)
                .then(node_key(&a.creation).cmp(&node_key(&b.creation)))
        });
    }
    let mut children: BTreeMap<Option<&str>, Vec<&str>> = BTreeMap::new();
    for category in categories.values() {
        let parent = category.parent_category_id.as_deref().filter(|parent| {
            *parent != category.category_id.as_ref() && categories.contains_key(parent)
        });
        children
            .entry(parent)
            .or_default()
            .push(&category.category_id);
    }
    for group in children.values_mut() {
        group.sort_by(|a, b| {
            let a = categories[a];
            let b = categories[b];
            a.order
                .cmp(&b.order)
                .then(a.title.cmp(&b.title))
                .then(a.category_id.cmp(&b.category_id))
        });
    }
    let mut rows = vec![];
    let mut visited = BTreeSet::new();
    // Iterative traversal bounds stack use even for malformed category hierarchies.
    let mut pending: Vec<_> = children
        .get(&None)
        .into_iter()
        .flatten()
        .rev()
        .map(|id| (*id, 0))
        .collect();
    while let Some((id, depth)) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let category = categories[id];
        rows.push(ActivityRow {
            id: id.into(),
            depth,
            content: ActivityRowContent::Category {
                label: ActivityText::Literal(category.title.to_string()),
                default_expanded: false,
                tools: vec![],
                count: None,
            },
        });
        if let Some(group) = items.get(id) {
            for item in group {
                let key = node_key(&item.creation);
                rows.push(ActivityRow {
                    id: format!("node:{key}"),
                    depth: depth + 1,
                    content: ActivityRowContent::Item(ActivityItem::Node {
                        key,
                        title: item.title.to_string(),
                        creation: item.creation.clone(),
                    }),
                });
            }
        }
        if let Some(group) = children.get(&Some(id)) {
            pending.extend(group.iter().rev().map(|id| (*id, depth + 1)));
        }
    }
    // Keep only populated branches; descendants must already have been considered.
    let mut populated_depth = None;
    let mut retained = vec![];
    for row in rows.into_iter().rev() {
        if matches!(row.content, ActivityRowContent::Category { .. })
            && populated_depth.is_none_or(|depth| depth <= row.depth)
        {
            continue;
        }
        populated_depth = Some(row.depth);
        retained.push(row);
    }
    retained.reverse();
    retained
}

fn node_key(creation: &NodeCreation) -> String {
    match creation {
        NodeCreation::Static { node_type_id } => format!("static:{node_type_id}"),
        NodeCreation::ParameterizedStatic { node_type_id, .. } => {
            format!("parameterizedStatic:{node_type_id}")
        }
        NodeCreation::ResourceBound {
            node_type_id,
            resource_path,
            ..
        } => format!("resourceBound:{node_type_id}:{}", resource_path.as_str()),
    }
}

pub fn commands_activity_panel() -> ActivityPanelDocument {
    let mut document = ActivityPanelDocument::new("commands", "activityBar.commands");
    document.empty_state = Some((
        ActivityText::Key("sidebar.noActiveGraph"),
        ActivityText::Key("sidebar.noActiveGraphDescription"),
    ));
    for (id, label) in [("undo", "common.undo"), ("redo", "common.redo")] {
        document.rows.push(ActivityRow {
            id: id.into(),
            depth: 0,
            content: ActivityRowContent::Item(ActivityItem::Command {
                id,
                label: ActivityText::Key(label),
            }),
        });
    }
    document
}

pub fn plugins_activity_panel(plugins: &[InstalledPlugin]) -> ActivityPanelDocument {
    let mut document = ActivityPanelDocument::new("plugins", "activityBar.plugins");
    document.tools = vec![
        ActivityTool {
            id: "install",
            label: ActivityText::Key("plugins.installPackage"),
            icon: "install",
        },
        ActivityTool {
            id: "refresh",
            label: ActivityText::Key("plugins.recheck"),
            icon: "refresh",
        },
    ];
    document.rows.push(ActivityRow {
        id: "plugins.installed".into(),
        depth: 0,
        content: ActivityRowContent::Category {
            label: ActivityText::Key("plugins.installed"),
            default_expanded: true,
            tools: vec![],
            count: Some(plugins.len()),
        },
    });
    document
        .rows
        .extend(plugins.iter().map(|plugin| ActivityRow {
            id: format!("plugin:{}", plugin.manifest.id),
            depth: 1,
            content: ActivityRowContent::Item(ActivityItem::Plugin {
                id: plugin.manifest.id.clone(),
                name: plugin.manifest.name.clone(),
                description: plugin.manifest.description.clone(),
                publisher: plugin.manifest.publisher.clone(),
                enabled: plugin.enabled,
            }),
        }));
    document
}
