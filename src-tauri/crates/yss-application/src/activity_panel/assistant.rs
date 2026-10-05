use yss_harness_contract::PrincipalId;
use yss_harness_core::HarnessHost;
use yss_project_identity::ProjectInstanceId;

use super::{
    ActivityItem, ActivityPanelDocument, ActivityRow, ActivityRowContent, ActivityText,
    ActivityTool,
};
use crate::{harness::HarnessSessionError, session::ApplicationState};

impl ApplicationState {
    /// Project-scoped presentation of persisted conversations; opening remains an explicit action.
    pub async fn assistant_activity_panel(
        &self,
        host: &HarnessHost,
        principal: &PrincipalId,
        project_instance_id: Option<ProjectInstanceId>,
    ) -> Result<ActivityPanelDocument, HarnessSessionError> {
        let captured = self.capture_session()?;
        if project_instance_id
            .as_ref()
            .is_some_and(|id| id != captured.project_instance_id())
        {
            return Err(HarnessSessionError::Changed);
        }
        let sessions = self.list_harness_sessions(host, principal).await?;
        self.revalidate_captured_session(&captured)
            .map_err(|_| HarnessSessionError::Changed)?;
        let mut document = ActivityPanelDocument::new("assistant", "panel.assistantConversations");
        document.project_instance_id = project_instance_id.map(|id| id.to_string());
        document.tools.push(ActivityTool {
            id: "newConversation",
            label: ActivityText::Key("panel.assistantNewConversation"),
            icon: "add",
        });
        document.rows = sessions
            .into_iter()
            .filter_map(|session| {
                let conversation = session.conversation?;
                Some(ActivityRow {
                    id: format!("conversation:{}", session.id),
                    depth: 0,
                    content: ActivityRowContent::Item(ActivityItem::Conversation {
                        session_id: session.id.to_string(),
                        title: conversation.title,
                        last_opened_at: conversation.last_opened_at.get(),
                    }),
                })
            })
            .collect();
        document.empty_state = Some((
            ActivityText::Key("panel.assistantConversations"),
            ActivityText::Key("panel.assistantNoConversations"),
        ));
        Ok(document)
    }
}
