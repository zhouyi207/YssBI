use serde::{Deserialize, Serialize};
use yss_application::activity_panel as app;

use super::catalog::NodeCreationDescriptorDto;
use crate::error::CommandError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivityPanelId {
    Project,
    Nodes,
    Commands,
    Plugins,
}
impl ActivityPanelId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Nodes => "nodes",
            Self::Commands => "commands",
            Self::Plugins => "plugins",
        }
    }
    pub fn is_project_scoped(self) -> bool {
        matches!(self, Self::Project | Self::Nodes)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityPanelRequest {
    pub panel_id: ActivityPanelId,
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivityGraphKindDto {
    Event,
    Function,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ActivityTextDto {
    Key { key: &'static str },
    Literal { text: String },
}
impl From<app::ActivityText> for ActivityTextDto {
    fn from(text: app::ActivityText) -> Self {
        match text {
            app::ActivityText::Key(key) => Self::Key { key },
            app::ActivityText::Literal(text) => Self::Literal { text },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ActivityToolDto {
    id: &'static str,
    label: ActivityTextDto,
    icon: &'static str,
}
impl From<app::ActivityTool> for ActivityToolDto {
    fn from(tool: app::ActivityTool) -> Self {
        Self {
            id: tool.id,
            label: tool.label.into(),
            icon: tool.icon,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ActivityItemDto {
    Graph {
        path: String,
        name: String,
        graph_type: ActivityGraphKindDto,
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
        creation: NodeCreationDescriptorDto,
    },
    Command {
        id: &'static str,
        label: ActivityTextDto,
    },
    Plugin {
        id: String,
        name: String,
        description: String,
        publisher: String,
        enabled: bool,
    },
}
impl From<app::ActivityItem> for ActivityItemDto {
    fn from(item: app::ActivityItem) -> Self {
        match item {
            app::ActivityItem::Graph {
                path,
                name,
                graph_type,
            } => Self::Graph {
                path,
                name,
                graph_type: match graph_type {
                    yss_graph_document::GraphResourceKind::Event => ActivityGraphKindDto::Event,
                    yss_graph_document::GraphResourceKind::Function => {
                        ActivityGraphKindDto::Function
                    }
                },
            },
            app::ActivityItem::Chart { path, name } => Self::Chart { path, name },
            app::ActivityItem::Database {
                id,
                name,
                resource_path,
            } => Self::Database {
                id,
                name,
                resource_path,
            },
            app::ActivityItem::Node {
                key,
                title,
                creation,
            } => Self::Node {
                key,
                title,
                creation: creation.into(),
            },
            app::ActivityItem::Command { id, label } => Self::Command {
                id,
                label: label.into(),
            },
            app::ActivityItem::Plugin {
                id,
                name,
                description,
                publisher,
                enabled,
            } => Self::Plugin {
                id,
                name,
                description,
                publisher,
                enabled,
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ActivityRowContentDto {
    Category {
        label: ActivityTextDto,
        default_expanded: bool,
        tools: Vec<ActivityToolDto>,
        count: Option<usize>,
    },
    Item {
        item: ActivityItemDto,
    },
    Message {
        label: ActivityTextDto,
        description: Option<ActivityTextDto>,
    },
}
#[derive(Debug, Serialize)]
pub struct ActivityRowDto {
    id: String,
    depth: usize,
    #[serde(flatten)]
    content: ActivityRowContentDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityPanelDocumentDto {
    schema: &'static str,
    panel_id: &'static str,
    project_instance_id: Option<String>,
    publication_revision: u64,
    title: ActivityTextDto,
    tools: Vec<ActivityToolDto>,
    rows: Vec<ActivityRowDto>,
    empty_state: Option<ActivityEmptyStateDto>,
}

#[derive(Debug, Serialize)]
struct ActivityEmptyStateDto {
    title: ActivityTextDto,
    description: ActivityTextDto,
}

impl TryFrom<app::ActivityPanelDocument> for ActivityPanelDocumentDto {
    type Error = CommandError;
    fn try_from(document: app::ActivityPanelDocument) -> Result<Self, Self::Error> {
        if document.rows.len() > 10_000 || document.rows.iter().any(|row| row.depth > 32) {
            return Err(CommandError::expected("activity_panel_limit_exceeded"));
        }
        let document = Self {
            schema: "yssbi.activity-panel.v1",
            panel_id: document.panel_id,
            project_instance_id: document.project_instance_id,
            publication_revision: document.publication_revision,
            title: document.title.into(),
            tools: document.tools.into_iter().map(Into::into).collect(),
            empty_state: document
                .empty_state
                .map(|(title, description)| ActivityEmptyStateDto {
                    title: title.into(),
                    description: description.into(),
                }),
            rows: document
                .rows
                .into_iter()
                .map(|row| ActivityRowDto {
                    id: row.id,
                    depth: row.depth,
                    content: match row.content {
                        app::ActivityRowContent::Category {
                            label,
                            default_expanded,
                            tools,
                            count,
                        } => ActivityRowContentDto::Category {
                            label: label.into(),
                            default_expanded,
                            tools: tools.into_iter().map(Into::into).collect(),
                            count,
                        },
                        app::ActivityRowContent::Item(item) => {
                            ActivityRowContentDto::Item { item: item.into() }
                        }
                        app::ActivityRowContent::Message { label, description } => {
                            ActivityRowContentDto::Message {
                                label: label.into(),
                                description: description.map(Into::into),
                            }
                        }
                    },
                })
                .collect(),
        };
        Ok(document)
    }
}
