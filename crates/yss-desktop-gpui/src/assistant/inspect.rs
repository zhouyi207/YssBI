//! Visible tool cards read the sanitized ledger projection within the original session boundary.
mod render;
use super::{
    ConversationPanel,
    projection::{Timing, Tool},
};
use crate::project::resources::ResourceCatalog;
use gpui_kit::{App, Context, IntoElement, RenderOnce, SharedString, Task, WeakEntity, Window};
use std::sync::Arc;
use yss_harness_contract::{
    AssistantToolIdentity, AssistantToolInspection, ProjectResourceRef, ToolInvocationId,
};

#[derive(IntoElement, Clone)]
pub(super) struct ToolCard {
    pub tool: Tool,
    pub owner: WeakEntity<ConversationPanel>,
    pub generation: u64,
    pub catalog: Option<Arc<ResourceCatalog>>,
    pub connected: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Loading {
    Pending,
    Reading,
    Ready,
    Failed,
}

#[derive(Clone, Copy, PartialEq)]
enum CopyTarget {
    Arguments,
    Details,
}

struct Inspection {
    card: ToolCard,
    open: bool,
    technical_open: bool,
    technical: Option<SharedString>,
    copied: Option<CopyTarget>,
    loading: Loading,
    detail: Option<AssistantToolInspection>,
    target: Option<ProjectResourceRef>,
    query: u64,
    task: Option<Task<()>>,
}

impl RenderOnce for ToolCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = SharedString::from(format!("tool-detail-{}", self.tool.id));
        let state = window.use_keyed_state(id, cx, |_, _| Inspection {
            card: self.clone(),
            open: false,
            technical_open: false,
            technical: None,
            copied: None,
            loading: Loading::Pending,
            detail: None,
            target: None,
            query: 0,
            task: None,
        });
        state.update(cx, |view, cx| {
            let rebound = view.card.owner.entity_id() != self.owner.entity_id()
                || view.card.tool.id != self.tool.id;
            if rebound || view.card.generation != self.generation || view.card.tool != self.tool {
                view.query += 1;
                view.task = None;
                view.detail = None;
                view.technical = None;
                view.copied = None;
                view.loading = Loading::Pending;
                if rebound {
                    view.open = false;
                    view.technical_open = false;
                }
            }
            let catalog_changed = match (&view.card.catalog, &self.catalog) {
                (Some(left), Some(right)) => !Arc::ptr_eq(left, right),
                (None, None) => false,
                _ => true,
            };
            view.card = self;
            if catalog_changed || view.detail.is_none() {
                view.resolve_target();
            }
            if view.loading == Loading::Pending
                && (view.open
                    || matches!(view.card.tool.kind, AssistantToolIdentity::Capability(_)))
            {
                view.loading = Loading::Reading;
                // RenderOnce is called while ConversationPanel is leased. Read it after rendering.
                cx.defer_in(window, |view, window, cx| view.load(window, cx));
            }
        });
        state
    }
}

impl Inspection {
    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.task.is_some() {
            return;
        }
        self.query += 1;
        let query = self.query;
        let Some(owner) = self.card.owner.upgrade() else {
            return;
        };
        let owner = owner.read(cx);
        if owner.generation != self.card.generation {
            return;
        }
        let Ok(invocation) = ToolInvocationId::try_new(self.card.tool.id.clone()) else {
            self.loading = Loading::Failed;
            cx.notify();
            return;
        };
        let services = owner.services.clone();
        let principal = owner.principal.clone();
        let session = owner.session.id.clone();
        let project = owner.session.project.clone();
        let generation = self.card.generation;
        let kind = self.card.tool.kind;
        let requested_session = session.clone();
        self.loading = Loading::Reading;
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
            let detail = match kind {
                AssistantToolIdentity::Capability(_) => services
                    .application
                    .harness
                    .host
                    .inspect_tool_invocation(&requested_session, &invocation)
                    .await
                    .map_err(|_| ())?
                    .map(AssistantToolInspection::from),
                AssistantToolIdentity::Control(_) => {
                    let events = services
                        .application
                        .harness
                        .host
                        .events_after(&requested_session, 0)
                        .await
                        .map_err(|_| ())?;
                    AssistantToolInspection::from_control_events(&events, &invocation)
                }
            };
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
            detail.ok_or(())
        });
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = job.await.unwrap_or(Err(()));
            let _ = view.update_in(cx, |view, _, cx| {
                if view.query != query {
                    return;
                }
                view.task = None;
                let current = view.card.owner.upgrade().is_some_and(|owner| {
                    let owner = owner.read(cx);
                    owner.generation == generation
                        && owner.session.id == session
                        && owner.session.project == project
                });
                if let Ok(detail) = result
                    && current
                {
                    view.detail = Some(detail);
                    view.loading = Loading::Ready;
                } else {
                    view.detail = None;
                    view.loading = Loading::Failed;
                }
                view.technical = None;
                view.copied = None;
                view.resolve_target();
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn resolve_target(&mut self) {
        self.target = None;
        let Some(id) = self
            .detail
            .as_ref()
            .and_then(|detail| detail.target.as_ref())
        else {
            return;
        };
        let Some(catalog) = &self.card.catalog else {
            return;
        };
        let mut matches = catalog
            .entries
            .iter()
            .filter(|entry| &entry.resource.id == id);
        if let Some(entry) = matches.next()
            && matches.next().is_none()
        {
            self.target = Some(entry.resource.clone());
        }
    }

    fn timing(&self) -> Option<Timing> {
        let event = self.card.tool.timing;
        self.detail
            .as_ref()
            .map(|detail| Timing {
                started_at: detail.started_at,
                updated_at: event.map_or(detail.started_at, |timing| timing.updated_at),
                finished_at: detail
                    .finished_at
                    .or(event.and_then(|timing| timing.finished_at)),
            })
            .or(event)
    }

    fn copy(&mut self, target: CopyTarget, cx: &mut Context<Self>) {
        let Some(detail) = &self.detail else { return };
        let text = match target {
            CopyTarget::Arguments => serde_json::to_string_pretty(&detail.parameters),
            CopyTarget::Details => serde_json::to_string_pretty(detail),
        }
        .expect("tool inspection contains JSON values");
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(text));
        self.copied = Some(target);
        cx.notify();
    }
}
