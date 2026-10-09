//! Recorded artifacts use current catalog metadata; opening stays with the workbench.
use super::{ConversationEvent, ConversationPanel};
use gpui::{AnyElement, App, IntoElement, SharedString, WeakEntity, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit_assets::IconName;
use yss_graph_execution::{
    identity::ExecutionSessionId,
    result::{ResultId, ResultReference},
};
use yss_harness_contract::{AssistantResultReference, ResourceChange};

pub(super) fn cards(
    scope: impl Into<SharedString>,
    owner: &WeakEntity<ConversationPanel>,
    artifacts: &[ResourceChange],
    results: &[AssistantResultReference],
    catalog: Option<&crate::project::resources::ResourceCatalog>,
    cx: &App,
) -> AnyElement {
    let mut cards = div().id(scope.into()).min_w_0().flex().flex_col().gap_2();
    for (index, artifact) in artifacts.iter().enumerate() {
        let current = catalog.and_then(|catalog| catalog.get(&artifact.resource));
        let unavailable = artifact.deleted || current.is_none();
        let updated = !unavailable
            && current
                .and_then(|entry| entry.revision(artifact.revision_kind))
                .is_some_and(|revision| revision != artifact.revision);
        let name = current
            .map(|entry| entry.name.as_str())
            .unwrap_or(&artifact.resource.id);
        let resource = artifact.resource.clone();
        let owner = owner.clone();
        cards = cards.child(
            div()
                .min_w_0()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .p_2()
                .child(
                    Button::new(("artifact", index))
                        .small()
                        .ghost()
                        .w_full()
                        .h_auto()
                        .justify_start()
                        .icon(IconName::FileText)
                        .accessibility_label(name.to_owned())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .whitespace_normal()
                                .child(name.to_owned()),
                        )
                        .tooltip(artifact.resource.id.clone())
                        .disabled(unavailable)
                        .on_click(move |_, _, cx| {
                            let _ = owner.update(cx, |view, cx| view.open_reference(&resource, cx));
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::text::t(if unavailable {
                            "panel.assistantResourceUnavailable"
                        } else {
                            "panel.assistantOpenResource"
                        })),
                )
                .when(updated, |card| {
                    card.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::text::t("panel.assistantResourceUpdated")),
                    )
                }),
        );
    }
    for (index, result) in results.iter().enumerate() {
        let reference = result_reference(result);
        let owner = owner.clone();
        cards = cards.child(
            div()
                .min_w_0()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .p_2()
                .child(
                    Button::new(("result", index))
                        .small()
                        .ghost()
                        .w_full()
                        .h_auto()
                        .justify_start()
                        .icon(IconName::Table)
                        .accessibility_label(result.output.clone())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .whitespace_normal()
                                .child(result.output.clone()),
                        )
                        .tooltip(result.output.clone())
                        .disabled(reference.is_none())
                        .on_click(move |_, _, cx| {
                            if let Some(reference) = reference {
                                let _ = owner.update(cx, |_, cx| {
                                    cx.emit(ConversationEvent::OpenResult(reference))
                                });
                            }
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::text::t(if reference.is_some() {
                            "panel.assistantOpenResult"
                        } else {
                            "panel.assistantResultUnavailable"
                        })),
                ),
        );
    }
    cards.into_any_element()
}

fn result_reference(result: &AssistantResultReference) -> Option<ResultReference> {
    Some(ResultReference {
        execution_session_id: ExecutionSessionId::new(result.execution_session_id.parse().ok()?),
        result_id: ResultId::from_existing(result.result_id.parse().ok()?),
    })
}
