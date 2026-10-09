//! Expanded worker output borrows the same timeline as the main conversation.
use super::super::{ConversationPanel, projection::Task};
use gpui::{AnyElement, Context, IntoElement, div, prelude::*};

impl ConversationPanel {
    pub(in crate::assistant) fn task_content(
        &self,
        id: u64,
        task: &Task,
        running: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut body = div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.render_output(&task.output, &[], running, cx));
        if let Some(summary) = &task.summary
            && !summary.is_empty()
            && !task.output.ends_with(summary)
        {
            body = body.child(super::output::markdown(
                format!("task-summary-{id}"),
                summary.clone(),
                false,
                cx,
            ));
        }
        for warning in &task.warnings {
            body = body.child(div().text_xs().child(warning.clone()));
        }
        body.into_any_element()
    }

    pub(super) fn outcome_card(&self, id: u64, task: &Task, cx: &mut Context<Self>) -> AnyElement {
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .children(
                task.warnings
                    .iter()
                    .map(|warning| div().text_xs().child(warning.clone())),
            )
            .when(
                !task.artifacts.is_empty() || !task.results.is_empty(),
                |body| {
                    body.child(super::super::resources::cards(
                        format!("outcome-{id}"),
                        &cx.entity().downgrade(),
                        &task.artifacts,
                        &task.results,
                        self.resource_catalog.as_deref(),
                        cx,
                    ))
                },
            )
            .into_any_element()
    }
}
