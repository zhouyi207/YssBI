//! Built-in role definitions, execution limits and compiled prompts.

use yss_harness_contract::{AgentRole, CapabilityId as Cap};

pub use policy::{authorize_agent_capability, authorize_agent_resource};
mod policy;

pub struct AgentDefinition {
    pub role: AgentRole,
    pub instructions: &'static str,
    pub capabilities: &'static [Cap],
}

impl AgentDefinition {
    pub fn limits(&self) -> yss_harness_contract::AgentRunLimits {
        yss_harness_contract::AgentRunLimits {
            maximum_model_turns: 32,
            maximum_output_tokens: 8192,
            maximum_duration_ms: if self.role == AgentRole::Manager {
                1_800_000
            } else {
                300_000
            },
            tool_concurrency: if self.role == AgentRole::Manager {
                4
            } else {
                1
            },
        }
    }

    pub fn control_tools(&self) -> Vec<yss_harness_contract::AgentControlTool> {
        use yss_harness_contract::AgentControlTool;
        match self.role {
            AgentRole::Manager => vec![AgentControlTool::DelegateTask],
            AgentRole::Stats => vec![AgentControlTool::ProposeStatisticalPlan],
            _ => vec![],
        }
    }

    pub fn output_mode(&self) -> yss_harness_contract::AgentOutputMode {
        if self.role == AgentRole::Manager {
            yss_harness_contract::AgentOutputMode::Transcript
        } else {
            yss_harness_contract::AgentOutputMode::FinalResponse
        }
    }
}

const READ: &[Cap] = &[
    Cap::InspectResource,
    Cap::InspectGraph,
    Cap::InspectDatasetSchema,
    Cap::InspectDatasetProfile,
    Cap::InspectResult,
    Cap::ListGraphResults,
];

pub static AGENT_DEFINITIONS: [AgentDefinition; 6] = [
    AgentDefinition {
        role: AgentRole::Manager,
        instructions: include_str!("prompts/manager.txt"),
        capabilities: &[
            Cap::InspectProject,
            Cap::InspectResource,
            Cap::InspectGraph,
            Cap::InspectDatasetSchema,
            Cap::InspectDatasetProfile,
            Cap::InspectResult,
            Cap::ListGraphResults,
            Cap::InspectUi,
            Cap::RequestUiIntent,
        ],
    },
    AgentDefinition {
        role: AgentRole::Data,
        instructions: include_str!("prompts/data.txt"),
        capabilities: &[
            Cap::InspectResource,
            Cap::InspectDatasetSchema,
            Cap::InspectDatasetProfile,
            Cap::ManageResource,
            Cap::EditResource,
            Cap::ExportDataset,
        ],
    },
    AgentDefinition {
        role: AgentRole::Stats,
        instructions: include_str!("prompts/stats.txt"),
        capabilities: &[
            Cap::InspectResource,
            Cap::InspectDatasetSchema,
            Cap::InspectDatasetProfile,
            Cap::InspectGraph,
            Cap::SearchNodeCatalog,
            Cap::ManageResource,
            Cap::EditResource,
            Cap::ApplyGraphEdit,
            Cap::ValidateGraph,
            Cap::ExecuteGraph,
            Cap::SaveGraph,
            Cap::ListGraphResults,
            Cap::InspectResult,
        ],
    },
    AgentDefinition {
        role: AgentRole::Plot,
        instructions: include_str!("prompts/plot.txt"),
        capabilities: &[
            Cap::InspectResource,
            Cap::InspectDatasetSchema,
            Cap::InspectResult,
            Cap::InspectGraph,
            Cap::ListGraphResults,
            Cap::ManageResource,
            Cap::EditResource,
        ],
    },
    AgentDefinition {
        role: AgentRole::Report,
        instructions: include_str!("prompts/report.txt"),
        capabilities: &[
            Cap::InspectResource,
            Cap::InspectResult,
            Cap::InspectGraph,
            Cap::ListGraphResults,
            Cap::ManageResource,
            Cap::EditResource,
        ],
    },
    AgentDefinition {
        role: AgentRole::Review,
        instructions: include_str!("prompts/review.txt"),
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
