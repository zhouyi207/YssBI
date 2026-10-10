//! Citation display state lives with the visible card; knowledge owners validate every read.
use super::{ConversationEvent, ConversationPanel};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
};
use gpui_kit::{
    App, Context, IntoElement, Render, RenderOnce, SharedString, Task, WeakEntity, Window, div,
    prelude::*, px,
};
use yss_harness_contract::{KnowledgeCitation, ProjectResourceRef};

#[derive(IntoElement)]
pub(super) struct SourceCard {
    id: SharedString,
    citation: KnowledgeCitation,
    owner: WeakEntity<ConversationPanel>,
}

pub(super) fn card(
    id: impl Into<SharedString>,
    citation: &KnowledgeCitation,
    owner: WeakEntity<ConversationPanel>,
) -> SourceCard {
    SourceCard {
        id: id.into(),
        citation: citation.clone(),
        owner,
    }
}

struct Detail {
    text: String,
    resource: Option<ProjectResourceRef>,
}

struct Source {
    owner: WeakEntity<ConversationPanel>,
    citation: KnowledgeCitation,
    open: bool,
    loading: bool,
    failed: bool,
    detail: Option<Detail>,
    task: Option<Task<()>>,
}

impl RenderOnce for SourceCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id, cx, |_, _| Source {
            owner: self.owner.clone(),
            citation: self.citation.clone(),
            open: false,
            loading: false,
            failed: false,
            detail: None,
            task: None,
        });
        state.update(cx, |view, cx| {
            if view.citation != self.citation || view.owner.entity_id() != self.owner.entity_id() {
                view.owner = self.owner;
                view.citation = self.citation;
                view.task = None;
                view.detail = None;
                view.open = false;
                view.loading = false;
                view.failed = false;
                cx.notify();
            }
        });
        state
    }
}

impl Source {
    fn load(&mut self, open_resource: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let owner = owner.read(cx);
        let services = owner.services.clone();
        let principal = owner.principal.clone();
        let session = owner.session.id.clone();
        let project = owner.session.project.clone();
        let citation = self.citation.clone();
        self.loading = true;
        self.failed = false;
        self.detail = None;
        let requested_session = session.clone();
        let job = services.executor.clone().spawn(async move {
            services
                .application
                .application
                .validate_harness_session(
                    &services.application.harness.host,
                    &principal,
                    &requested_session,
                )
                .await
                .map_err(|_| ())?;
            let text = services
                .application
                .harness
                .host
                .inspect_citation(&requested_session, &citation)
                .await
                .map_err(|_| ())?
                .ok_or(())?;
            let resource = services
                .application
                .harness
                .knowledge
                .citation_resource(&citation)
                .await
                .map_err(|_| ())?;
            services
                .application
                .application
                .validate_harness_session(
                    &services.application.harness.host,
                    &principal,
                    &requested_session,
                )
                .await
                .map_err(|_| ())?;
            Ok::<_, ()>(Detail { text, resource })
        });
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = job.await.unwrap_or(Err(()));
            let _ = view.update_in(cx, |view, _, cx| {
                view.loading = false;
                view.task = None;
                let current = view.owner.upgrade().is_some_and(|owner| {
                    let owner = owner.read(cx);
                    owner.session.id == session && owner.session.project == project
                });
                if let Ok(detail) = result
                    && current
                {
                    if open_resource && let Some(resource) = &detail.resource {
                        let _ = view.owner.update(cx, |_, cx| {
                            cx.emit(ConversationEvent::OpenResource(resource.clone()));
                        });
                    }
                    view.detail = Some(detail);
                } else {
                    view.failed = true;
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}

impl Render for Source {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = div().min_w_0().flex().flex_col().gap_2().pt_2();
        if self.loading {
            content = content.child(
                div()
                    .text_xs()
                    .child(crate::text::t("panel.assistantLoadingDetails")),
            );
        }
        if self.failed {
            content = content
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(crate::text::t("panel.assistantCitationUnavailable")),
                )
                .child(
                    Button::new("retry-source")
                        .self_start()
                        .small()
                        .ghost()
                        .label(crate::text::t("panel.assistantRetryDetails"))
                        .on_click(cx.listener(|view, _, window, cx| view.load(false, window, cx))),
                );
        }
        if let Some(detail) = &self.detail {
            if detail.resource.is_some() {
                content = content.child(
                    Button::new("open-source")
                        .self_start()
                        .small()
                        .ghost()
                        .icon(IconName::FileText)
                        .label(crate::text::t("panel.assistantOpenSource"))
                        .on_click(cx.listener(|view, _, window, cx| view.load(true, window, cx))),
                );
            }
            content = content.child(
                div()
                    .id("source-text")
                    .min_w_0()
                    .max_h(px(360.))
                    .overflow_y_scroll()
                    .child(super::markdown::view(
                        "citation-text",
                        detail.text.clone(),
                        false,
                        self.owner.clone(),
                    )),
            );
        }
        Collapsible::new()
            .open(self.open)
            .min_w_0()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::t("panel.assistantSource")),
            )
            .child(
                Button::new("toggle-source")
                    .small()
                    .ghost()
                    .w_full()
                    .h_auto()
                    .justify_start()
                    .icon(if self.open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .accessibility_label(self.citation.title.clone())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .whitespace_normal()
                            .child(self.citation.title.clone()),
                    )
                    .tooltip(self.citation.title.clone())
                    .disabled(self.loading)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.open = !view.open;
                        if view.open {
                            view.load(false, window, cx);
                        }
                        cx.notify();
                    })),
            )
            .content(content)
    }
}
