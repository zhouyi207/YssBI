//! The JSON form of Assistant tool requests and results in the Harness ledger, and their model projection.

use serde_json::{Value, json};

use crate::{
    AutomationCapabilityRequest, AutomationCapabilityResult, KnowledgePassage,
    KnowledgeSearchResult, READ_KNOWLEDGE_TOOL, SEARCH_KNOWLEDGE_TOOL,
};

/// Plan proposals are validated by the Assistant extension and never reach Application owners.
pub const PROPOSE_STATISTICAL_PLAN_TOOL: &str = "propose_statistical_plan";

pub fn encode_request(request: &AutomationCapabilityRequest) -> Result<Value, serde_json::Error> {
    serde_json::to_value(request)
}

pub fn decode_request(value: &Value) -> Result<AutomationCapabilityRequest, serde_json::Error> {
    serde_json::from_value(value.clone())
}

pub fn encode_result(result: &AutomationCapabilityResult) -> Result<Value, serde_json::Error> {
    serde_json::to_value(result)
}

pub fn decode_result(value: &Value) -> Result<AutomationCapabilityResult, serde_json::Error> {
    serde_json::from_value(value.clone())
}

/// The model-facing projection of any recorded Assistant tool result.
pub fn project_tool_result(tool: &str, result: &Value) -> Result<Value, serde_json::Error> {
    match tool {
        SEARCH_KNOWLEDGE_TOOL => {
            let result: KnowledgeSearchResult = serde_json::from_value(result.clone())?;
            Ok(json!({"type": "knowledge_search", "payload": result}))
        }
        READ_KNOWLEDGE_TOOL => {
            let result: KnowledgePassage = serde_json::from_value(result.clone())?;
            Ok(json!({"type": "knowledge_passage", "payload": result}))
        }
        PROPOSE_STATISTICAL_PLAN_TOOL => Ok(result.clone()),
        _ => crate::model::capability_result(&decode_result(result)?),
    }
}
