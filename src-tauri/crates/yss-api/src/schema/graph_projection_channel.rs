use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::application_event::{GraphProjectionReplacementDto, ProjectionStatusDto};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct GraphProjectionPublicationKey {
    pub project_instance_id: String,
    pub graph_session_id: String,
    pub graph_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphProjectionPublicationDto {
    pub project_instance_id: String,
    pub graph_session_id: String,
    pub graph_path: String,
    pub request_generation: u64,
    pub replacement: GraphProjectionReplacementDto,
}

impl GraphProjectionPublicationDto {
    pub(crate) fn key(&self) -> GraphProjectionPublicationKey {
        GraphProjectionPublicationKey {
            project_instance_id: self.project_instance_id.clone(),
            graph_session_id: self.graph_session_id.clone(),
            graph_path: self.graph_path.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum GraphProjectionChannelEventDto {
    #[serde(rename = "projectionReplaced")]
    Replaced {
        project_instance_id: String,
        graph_session_id: String,
        graph_path: String,
        request_generation: u64,
        replacement: Box<GraphProjectionReplacementDto>,
    },
    #[serde(rename = "projectionBatchReplaced")]
    BatchReplaced {
        project_instance_id: String,
        publication_revision: u64,
        replacements: Vec<GraphProjectionPublicationDto>,
        status: ProjectionStatusDto,
    },
    #[serde(rename = "projectionInvalidated")]
    Invalidated {
        project_instance_id: String,
        graph_session_id: String,
        graph_path: String,
        request_generation: u64,
        reason_code: String,
        incident_id: Option<String>,
    },
}

impl GraphProjectionChannelEventDto {
    pub(crate) fn coalescing_key(&self) -> String {
        match self {
            Self::Replaced {
                project_instance_id,
                graph_session_id,
                graph_path,
                ..
            }
            | Self::Invalidated {
                project_instance_id,
                graph_session_id,
                graph_path,
                ..
            } => format!("{project_instance_id}\0{graph_session_id}\0{graph_path}"),
            Self::BatchReplaced {
                project_instance_id,
                publication_revision,
                ..
            } => format!("{project_instance_id}\0batch\0{publication_revision}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphProjectionSnapshotDto {
    pub project_instance_id: String,
    pub stream_id: String,
    pub projections: Vec<GraphProjectionPublicationDto>,
    pub latest_generation_by_graph: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphProjectionSubscriptionDto {
    pub subscription_id: String,
    pub snapshot: GraphProjectionSnapshotDto,
}
