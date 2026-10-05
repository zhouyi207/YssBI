//! Built-in role definitions, tool concurrency and compiled prompts.

use yss_harness_contract::{AgentRole, CapabilityId as Cap};

pub use policy::{authorize_agent_capability, authorize_agent_resource};
pub(crate) use policy::{authorize_model_capability, validate_agent_read};
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

    pub fn control_tools(&self) -> Vec<yss_harness_contract::AgentControlTool> {
        use yss_harness_contract::AgentControlTool;
        match self.role {
            AgentRole::Manager => vec![
                AgentControlTool::DelegateTask,
                AgentControlTool::FollowupTask,
            ],
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
    Cap::SearchKnowledge,
    Cap::ReadKnowledge,
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
        instructions: include_str!("prompts/manager.md"),
        capabilities: &[
            Cap::SearchKnowledge,
            Cap::ReadKnowledge,
            Cap::InspectProject,
            Cap::InspectResource,
            Cap::InspectGraph,
            Cap::InspectDatasetSchema,
            Cap::InspectDatasetProfile,
            Cap::InspectResult,
            Cap::ListGraphResults,
            Cap::InspectUiIntent,
            Cap::RequestUiIntent,
        ],
    },
    AgentDefinition {
        role: AgentRole::Data,
        instructions: include_str!("prompts/data.md"),
        capabilities: &[
            Cap::SearchKnowledge,
            Cap::ReadKnowledge,
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
        instructions: include_str!("prompts/stats.md"),
        capabilities: &[
            Cap::SearchKnowledge,
            Cap::ReadKnowledge,
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
        instructions: include_str!("prompts/plot.md"),
        capabilities: &[
            Cap::SearchKnowledge,
            Cap::ReadKnowledge,
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
        instructions: include_str!("prompts/report.md"),
        capabilities: &[
            Cap::SearchKnowledge,
            Cap::ReadKnowledge,
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
        instructions: include_str!("prompts/review.md"),
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
