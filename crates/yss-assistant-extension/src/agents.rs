//! Built-in YssBI role definitions, tool sets, concurrency and compiled prompts.

use crate::contract::AgentOutputMode;
use yss_assistant_contract::{AgentRole, CapabilityId as Cap};

pub use policy::{authorize_agent_capability, authorize_agent_resource};
pub(crate) use policy::{
    authorize_model_capability, resource_version_matches, validate_agent_read,
};
mod policy;

pub struct AgentDefinition {
    pub role: AgentRole,
    pub instructions: &'static str,
    pub capabilities: &'static [Cap],
}

impl AgentDefinition {
    pub fn tool_concurrency(&self) -> usize {
        if self.role == AgentRole::Manager {
            4
        } else {
            1
        }
    }

    pub fn delegates(&self) -> bool {
        self.role == AgentRole::Manager
    }

    /// Only the analysis role records statistical plans before execution.
    pub fn proposes_plans(&self) -> bool {
        self.role == AgentRole::Stats
    }

    pub fn output_mode(&self) -> AgentOutputMode {
        if self.role == AgentRole::Manager {
            AgentOutputMode::Transcript
        } else {
            AgentOutputMode::FinalResponse
        }
    }
}

const READ: &[Cap] = &[
    Cap::SearchKnowledge,
    Cap::ReadKnowledge,
    Cap::InspectResource,
    Cap::InspectChart,
    Cap::InspectDocument,
    Cap::ReadDocument,
    Cap::SearchDocument,
    Cap::InspectMind,
    Cap::FindTopics,
    Cap::InspectTopics,
    Cap::InspectGraph,
    Cap::ValidateGraph,
    Cap::FindNodes,
    Cap::FindConstants,
    Cap::InspectConstants,
    Cap::InspectNodes,
    Cap::FindConnections,
    Cap::InspectDatabase,
    Cap::InspectDatabaseSchema,
    Cap::ReadDatabaseRows,
    Cap::ProfileDatabase,
    Cap::InspectResult,
    Cap::ReadResultTable,
    Cap::ListGraphResults,
];

pub static AGENT_DEFINITIONS: [AgentDefinition; 6] = [
    AgentDefinition {
        role: AgentRole::Manager,
        instructions: include_str!("agents/prompts/manager.md"),
        capabilities: &[
            Cap::SearchKnowledge,
            Cap::ReadKnowledge,
            Cap::ListResources,
            Cap::InspectResource,
            Cap::InspectChart,
            Cap::InspectDocument,
            Cap::ReadDocument,
            Cap::SearchDocument,
            Cap::InspectMind,
            Cap::FindTopics,
            Cap::InspectTopics,
            Cap::InspectGraph,
            Cap::ValidateGraph,
            Cap::FindNodes,
            Cap::FindConstants,
            Cap::InspectConstants,
            Cap::InspectNodes,
            Cap::FindConnections,
            Cap::InspectDatabase,
            Cap::InspectDatabaseSchema,
            Cap::ReadDatabaseRows,
            Cap::ProfileDatabase,
            Cap::InspectResult,
            Cap::ReadResultTable,
            Cap::ListGraphResults,
            Cap::InspectUiIntent,
            Cap::RequestUiIntent,
        ],
    },
    AgentDefinition {
        role: AgentRole::Data,
        instructions: include_str!("agents/prompts/data.md"),
        capabilities: &[
            Cap::SearchKnowledge,
            Cap::ReadKnowledge,
            Cap::InspectResource,
            Cap::InspectChart,
            Cap::InspectDocument,
            Cap::ReadDocument,
            Cap::SearchDocument,
            Cap::InspectMind,
            Cap::FindTopics,
            Cap::InspectTopics,
            Cap::InspectDatabase,
            Cap::InspectDatabaseSchema,
            Cap::ReadDatabaseRows,
            Cap::ProfileDatabase,
            Cap::ImportDatabase,
            Cap::InsertRows,
            Cap::UpdateCells,
            Cap::DeleteRows,
            Cap::CreateColumns,
            Cap::RenameColumns,
            Cap::DeleteColumns,
            Cap::CastColumns,
            Cap::SetColumnSemantics,
            Cap::RenameResource,
            Cap::DuplicateResource,
            Cap::DeleteResource,
            Cap::SaveResource,
            Cap::UndoResource,
            Cap::RedoResource,
            Cap::ExportDatabase,
        ],
    },
    AgentDefinition {
        role: AgentRole::Stats,
        instructions: include_str!("agents/prompts/stats.md"),
        capabilities: &[
            Cap::SearchKnowledge,
            Cap::ReadKnowledge,
            Cap::InspectResource,
            Cap::InspectChart,
            Cap::InspectDocument,
            Cap::ReadDocument,
            Cap::SearchDocument,
            Cap::InspectMind,
            Cap::FindTopics,
            Cap::InspectTopics,
            Cap::InspectDatabase,
            Cap::InspectDatabaseSchema,
            Cap::ReadDatabaseRows,
            Cap::ProfileDatabase,
            Cap::InspectGraph,
            Cap::FindNodes,
            Cap::FindConstants,
            Cap::InspectConstants,
            Cap::InspectNodes,
            Cap::FindConnections,
            Cap::BrowseNodes,
            Cap::InspectNodeType,
            Cap::CreateResource,
            Cap::RenameResource,
            Cap::DuplicateResource,
            Cap::DeleteResource,
            Cap::SaveResource,
            Cap::EditResource,
            Cap::UndoResource,
            Cap::RedoResource,
            Cap::CreateNodes,
            Cap::CreateConstants,
            Cap::UpdateConstants,
            Cap::DeleteConstants,
            Cap::UpdateNodes,
            Cap::DeleteNodes,
            Cap::DuplicateNodes,
            Cap::MoveNodes,
            Cap::CreateConnections,
            Cap::UpdateConnections,
            Cap::DeleteConnections,
            Cap::ValidateGraph,
            Cap::ExecuteGraph,
            Cap::ListGraphResults,
            Cap::InspectResult,
            Cap::ReadResultTable,
        ],
    },
    AgentDefinition {
        role: AgentRole::Plot,
        instructions: include_str!("agents/prompts/plot.md"),
        capabilities: &[
            Cap::SearchKnowledge,
            Cap::ReadKnowledge,
            Cap::InspectResource,
            Cap::InspectChart,
            Cap::InspectDocument,
            Cap::ReadDocument,
            Cap::SearchDocument,
            Cap::InspectMind,
            Cap::FindTopics,
            Cap::InspectTopics,
            Cap::InspectDatabase,
            Cap::InspectDatabaseSchema,
            Cap::ReadDatabaseRows,
            Cap::InspectResult,
            Cap::ReadResultTable,
            Cap::InspectGraph,
            Cap::FindNodes,
            Cap::FindConstants,
            Cap::InspectConstants,
            Cap::InspectNodes,
            Cap::FindConnections,
            Cap::ListGraphResults,
            Cap::CreateResource,
            Cap::RenameResource,
            Cap::DuplicateResource,
            Cap::DeleteResource,
            Cap::SaveResource,
            Cap::UpdateChart,
        ],
    },
    AgentDefinition {
        role: AgentRole::Report,
        instructions: include_str!("agents/prompts/report.md"),
        capabilities: &[
            Cap::SearchKnowledge,
            Cap::ReadKnowledge,
            Cap::InspectResource,
            Cap::InspectDatabase,
            Cap::InspectDatabaseSchema,
            Cap::InspectChart,
            Cap::InspectDocument,
            Cap::ReadDocument,
            Cap::SearchDocument,
            Cap::InspectMind,
            Cap::FindTopics,
            Cap::InspectTopics,
            Cap::InspectResult,
            Cap::ReadResultTable,
            Cap::InspectGraph,
            Cap::FindNodes,
            Cap::FindConstants,
            Cap::InspectConstants,
            Cap::InspectNodes,
            Cap::FindConnections,
            Cap::ListGraphResults,
            Cap::CreateResource,
            Cap::RenameResource,
            Cap::DuplicateResource,
            Cap::DeleteResource,
            Cap::SaveResource,
            Cap::ReplaceDocumentText,
            Cap::AppendDocument,
            Cap::WriteDocument,
            Cap::CreateTopics,
            Cap::UpdateTopics,
            Cap::MoveTopics,
            Cap::DeleteTopics,
            Cap::DuplicateTopics,
        ],
    },
    AgentDefinition {
        role: AgentRole::Review,
        instructions: include_str!("agents/prompts/review.md"),
        capabilities: READ,
    },
];

pub fn agent_definition(role: AgentRole) -> &'static AgentDefinition {
    &AGENT_DEFINITIONS[match role {
        AgentRole::Manager => 0,
        AgentRole::Data => 1,
        AgentRole::Stats => 2,
        AgentRole::Plot => 3,
        AgentRole::Report => 4,
        AgentRole::Review => 5,
    }]
}
