//! Invalid provider tool names are model feedback, never executable aliases.
use rig_agent::agent::hook::{
    AgentHook, HookContext, InvalidToolCallAction, InvalidToolCallContext, InvalidToolCallReason,
};

pub(crate) struct ToolCallFeedback;

impl AgentHook for ToolCallFeedback {
    async fn on_invalid_tool_call(
        &self,
        _context: &HookContext,
        event: &InvalidToolCallContext,
    ) -> Option<InvalidToolCallAction> {
        let reason = match event.reason {
            InvalidToolCallReason::UnknownTool => "tool_not_available",
            InvalidToolCallReason::MalformedArguments { .. } => "malformed_tool_arguments",
        };
        // Arguments/history may contain user data; record only structural diagnostics.
        tracing::warn!(domain = "Application", event = "harness_invalid_tool_call",
            reason, tool_name = %event.tool_name, "Harness rejected a provider tool call");
        if !matches!(event.reason, InvalidToolCallReason::UnknownTool) {
            return None;
        }
        Some(InvalidToolCallAction::skip(serde_json::json!({
            "state": "failed",
            "failure": {
                "code": "invalid_request",
                "details": {
                    "reason": reason,
                    "nextStep": "Use a tool in the current definitions. Do not repeat an unavailable tool or infer access from another role's history. If no permitted tool can perform the task, report the missing capability.",
                    "allowedTools": event.allowed_tools,
                },
            },
        }).to_string()))
    }
}
