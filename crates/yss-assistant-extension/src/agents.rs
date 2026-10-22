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
    Cap::KnowledgeSearch,
    Cap::KnowledgeRead,
    Cap::WebSearch,
    Cap::WebFetch,
    Cap::ProjectInspectResource,
    Cap::ChartInspect,
    Cap::DocumentInspect,
    Cap::DocumentRead,
    Cap::DocumentSearch,
    Cap::MindInspect,
    Cap::MindSearchTopics,
    Cap::MindInspectTopics,
    Cap::GraphInspect,
    Cap::GraphValidate,
    Cap::GraphSearchNodes,
    Cap::GraphSearchConstants,
    Cap::GraphInspectConstants,
    Cap::GraphInspectNodes,
    Cap::GraphSearchConnections,
    Cap::DataInspect,
    Cap::DataInspectSchema,
    Cap::DataReadRows,
    Cap::DataInspectProfile,
    Cap::ResultInspect,
    Cap::ResultReadTable,
    Cap::ResultList,
];

pub static AGENT_DEFINITIONS: [AgentDefinition; 6] = [
    AgentDefinition {
        role: AgentRole::Manager,
        instructions: include_str!("agents/prompts/manager.md"),
        capabilities: &[
            Cap::KnowledgeSearch,
            Cap::KnowledgeRead,
            Cap::WebSearch,
            Cap::WebFetch,
            Cap::ProjectListResources,
            Cap::ProjectInspectResource,
            Cap::ChartInspect,
            Cap::DocumentInspect,
            Cap::DocumentRead,
            Cap::DocumentSearch,
            Cap::MindInspect,
            Cap::MindSearchTopics,
            Cap::MindInspectTopics,
            Cap::GraphInspect,
            Cap::GraphValidate,
            Cap::GraphSearchNodes,
            Cap::GraphSearchConstants,
            Cap::GraphInspectConstants,
            Cap::GraphInspectNodes,
            Cap::GraphSearchConnections,
            Cap::DataInspect,
            Cap::DataInspectSchema,
            Cap::DataReadRows,
            Cap::DataInspectProfile,
            Cap::ResultInspect,
            Cap::ResultReadTable,
            Cap::ResultList,
            Cap::UiInspectReceipt,
            Cap::UiOpen,
            Cap::PythonInspectEnvironment,
            Cap::PythonReadScript,
        ],
    },
    AgentDefinition {
        role: AgentRole::Data,
        instructions: include_str!("agents/prompts/data.md"),
        capabilities: &[
            Cap::KnowledgeSearch,
            Cap::KnowledgeRead,
            Cap::WebSearch,
            Cap::WebFetch,
            Cap::ProjectInspectResource,
            Cap::ChartInspect,
            Cap::DocumentInspect,
            Cap::DocumentRead,
            Cap::DocumentSearch,
            Cap::MindInspect,
            Cap::MindSearchTopics,
            Cap::MindInspectTopics,
            Cap::DataInspect,
            Cap::DataInspectSchema,
            Cap::DataReadRows,
            Cap::DataInspectProfile,
            Cap::DataImport,
            Cap::DataCreateRows,
            Cap::DataUpdateCells,
            Cap::DataDeleteRows,
            Cap::DataCreateColumns,
            Cap::DataRenameColumns,
            Cap::DataDeleteColumns,
            Cap::DataCastColumns,
            Cap::DataUpdateColumnSemantics,
            Cap::ProjectRenameResource,
            Cap::ProjectDuplicateResource,
            Cap::ProjectDeleteResource,
            Cap::ProjectSaveResource,
            Cap::ProjectUndoResource,
            Cap::ProjectRedoResource,
            Cap::DataExport,
            Cap::PythonInspectEnvironment,
            Cap::PythonReadScript,
            Cap::PythonWriteScript,
            Cap::PythonInstallPackages,
            Cap::PythonRunScript,
        ],
    },
    AgentDefinition {
        role: AgentRole::Stats,
        instructions: include_str!("agents/prompts/stats.md"),
        capabilities: &[
            Cap::KnowledgeSearch,
            Cap::KnowledgeRead,
            Cap::WebSearch,
            Cap::WebFetch,
            Cap::ProjectInspectResource,
            Cap::ChartInspect,
            Cap::DocumentInspect,
            Cap::DocumentRead,
            Cap::DocumentSearch,
            Cap::MindInspect,
            Cap::MindSearchTopics,
            Cap::MindInspectTopics,
            Cap::DataInspect,
            Cap::DataInspectSchema,
            Cap::DataReadRows,
            Cap::DataInspectProfile,
            Cap::GraphInspect,
            Cap::GraphSearchNodes,
            Cap::GraphSearchConstants,
            Cap::GraphInspectConstants,
            Cap::GraphInspectNodes,
            Cap::GraphSearchConnections,
            Cap::CatalogSearchNodes,
            Cap::CatalogInspectNodes,
            Cap::ProjectCreateResource,
            Cap::ProjectRenameResource,
            Cap::ProjectDuplicateResource,
            Cap::ProjectDeleteResource,
            Cap::ProjectSaveResource,
            Cap::GraphUpdateFunctionSignature,
            Cap::ProjectUndoResource,
            Cap::ProjectRedoResource,
            Cap::GraphCreateNodes,
            Cap::GraphCreateConstants,
            Cap::GraphUpdateConstants,
            Cap::GraphDeleteConstants,
            Cap::GraphUpdateNodes,
            Cap::GraphDeleteNodes,
            Cap::GraphDuplicateNodes,
            Cap::GraphMoveNodes,
            Cap::GraphCreateConnections,
            Cap::GraphUpdateConnections,
            Cap::GraphDeleteConnections,
            Cap::GraphValidate,
            Cap::GraphExecute,
            Cap::ResultList,
            Cap::ResultInspect,
            Cap::ResultReadTable,
        ],
    },
    AgentDefinition {
        role: AgentRole::Plot,
        instructions: include_str!("agents/prompts/plot.md"),
        capabilities: &[
            Cap::KnowledgeSearch,
            Cap::KnowledgeRead,
            Cap::WebSearch,
            Cap::WebFetch,
            Cap::ProjectInspectResource,
            Cap::ChartInspect,
            Cap::DocumentInspect,
            Cap::DocumentRead,
            Cap::DocumentSearch,
            Cap::MindInspect,
            Cap::MindSearchTopics,
            Cap::MindInspectTopics,
            Cap::DataInspect,
            Cap::DataInspectSchema,
            Cap::DataReadRows,
            Cap::ResultInspect,
            Cap::ResultReadTable,
            Cap::GraphInspect,
            Cap::GraphSearchNodes,
            Cap::GraphSearchConstants,
            Cap::GraphInspectConstants,
            Cap::GraphInspectNodes,
            Cap::GraphSearchConnections,
            Cap::ResultList,
            Cap::ProjectCreateResource,
            Cap::ProjectRenameResource,
            Cap::ProjectDuplicateResource,
            Cap::ProjectDeleteResource,
            Cap::ProjectSaveResource,
            Cap::ChartUpdate,
        ],
    },
    AgentDefinition {
        role: AgentRole::Report,
        instructions: include_str!("agents/prompts/report.md"),
        capabilities: &[
            Cap::KnowledgeSearch,
            Cap::KnowledgeRead,
            Cap::WebSearch,
            Cap::WebFetch,
            Cap::ProjectInspectResource,
            Cap::DataInspect,
            Cap::DataInspectSchema,
            Cap::ChartInspect,
            Cap::DocumentInspect,
            Cap::DocumentRead,
            Cap::DocumentSearch,
            Cap::MindInspect,
            Cap::MindSearchTopics,
            Cap::MindInspectTopics,
            Cap::ResultInspect,
            Cap::ResultReadTable,
            Cap::GraphInspect,
            Cap::GraphSearchNodes,
            Cap::GraphSearchConstants,
            Cap::GraphInspectConstants,
            Cap::GraphInspectNodes,
            Cap::GraphSearchConnections,
            Cap::ResultList,
            Cap::ProjectCreateResource,
            Cap::ProjectRenameResource,
            Cap::ProjectDuplicateResource,
            Cap::ProjectDeleteResource,
            Cap::ProjectSaveResource,
            Cap::DocumentReplaceText,
            Cap::DocumentAppendText,
            Cap::DocumentWriteText,
            Cap::MindCreateTopics,
            Cap::MindUpdateTopics,
            Cap::MindMoveTopics,
            Cap::MindDeleteTopics,
            Cap::MindDuplicateTopics,
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
