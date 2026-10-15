//! Creation owns only an uncommitted form. Application prepares its conditional fields.
mod ports;
mod query;
mod render;

use super::{
    graph_properties::GraphProperties,
    parameters::{ParameterChange, ParameterForm},
};
use crate::{GraphServices, canvas::GraphCanvas};
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, Subscription, Task, WeakEntity, Window,
};
use query::Preparation;
use std::sync::Arc;
use yss_application::graph::catalog::NodeCreationForm;
use yss_node_catalog::NodeCreation;
use yss_node_protocol::{InitialPortCounts, ParameterValues};
use yss_project::GraphEditVersion;
use yss_project_identity::ProjectInstanceId;

pub(crate) struct CreationTarget {
    pub graph: WeakEntity<GraphCanvas>,
    pub project: ProjectInstanceId,
    pub version: GraphEditVersion,
    pub descriptor: NodeCreation,
    pub title: String,
}

pub(crate) enum CreationEvent {
    Back,
    Create {
        parameters: ParameterValues,
        port_counts: InitialPortCounts,
    },
}

pub(crate) struct NodeCreationView {
    services: Arc<GraphServices>,
    target: CreationTarget,
    parameters: Entity<ParameterForm>,
    properties: Entity<GraphProperties>,
    form: Option<NodeCreationForm>,
    ports: Vec<ports::PortCount>,
    preparing: Option<Preparation>,
    creating: bool,
    language: &'static str,
    generation: u64,
    notifications: yss_ui::notifications::Notifications,
    task: Option<Task<()>>,
    _parameter_subscription: Subscription,
    _target_subscription: Option<Subscription>,
}

impl NodeCreationView {
    pub(crate) fn new(
        services: Arc<GraphServices>,
        target: CreationTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let properties = cx.new(|_| GraphProperties::new(services.clone(), window));
        let host = cx.entity();
        let owner = host.downgrade();
        let parameters = cx.new(|cx| {
            ParameterForm::new(
                properties.clone(),
                &host,
                move |cx| {
                    owner
                        .upgrade()
                        .is_some_and(|owner| owner.read(cx).can_edit(cx))
                },
                window,
                cx,
            )
        });
        let subscription = cx.subscribe_in(
            &parameters,
            window,
            |view, _, change: &ParameterChange, window, cx| {
                if !view.can_edit(cx) {
                    return;
                }
                let Some(form) = &view.form else {
                    return;
                };
                let mut values = form.values.clone();
                values.insert(change.key.clone(), change.value.clone());
                view.query(
                    Preparation {
                        values,
                        counts: form.port_counts.clone(),
                        field: Some(change.key.clone()),
                        create: false,
                    },
                    window,
                    cx,
                );
            },
        );
        let target_subscription = target.graph.upgrade().map(|graph| {
            let mut current = true;
            cx.observe(&graph, move |view: &mut Self, _, cx| {
                let next = view.current(cx);
                if current && !next && !view.creating {
                    view.notifications
                        .error(yss_ui::text::t("native.workbench.creationChanged"), cx);
                }
                current = next;
                cx.notify();
            })
        });
        let mut view = Self {
            services,
            target,
            parameters,
            properties,
            form: None,
            ports: vec![],
            preparing: None,
            creating: false,
            language: yss_ui::text::locale(),
            generation: 0,
            notifications: yss_ui::notifications::Notifications::new(window),
            task: None,
            _parameter_subscription: subscription,
            _target_subscription: target_subscription,
        };
        view.query(Preparation::default(), window, cx);
        view
    }

    fn current(&self, cx: &App) -> bool {
        self.target.graph.upgrade().is_some_and(|graph| {
            let graph = graph.read(cx);
            graph.graph.project == self.target.project
                && graph.graph.editing.version == self.target.version
        })
    }

    fn can_edit(&self, cx: &App) -> bool {
        self.preparing.is_none()
            && !self.creating
            && self.current(cx)
            && self
                .target
                .graph
                .upgrade()
                .is_some_and(|graph| !graph.read(cx).busy())
    }

    pub(crate) fn localize_title(
        &mut self,
        catalog: &yss_application::activity_panel::ActivityPanelDocument,
        cx: &mut Context<Self>,
    ) {
        use yss_application::activity_panel::{ActivityItem, ActivityRowContent};
        if let Some(title) = catalog.rows.iter().find_map(|row| match &row.content {
            ActivityRowContent::Item(ActivityItem::Node {
                creation, title, ..
            }) if creation == &self.target.descriptor => Some(title),
            _ => None,
        }) && title != &self.target.title
        {
            self.target.title = title.clone();
            cx.notify();
        }
    }

    pub(crate) fn creation_failed(&mut self, cx: &mut Context<Self>) {
        self.creating = false;
        cx.notify();
    }

    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit(cx) {
            return;
        }
        let Some(changes) = self
            .parameters
            .update(cx, |parameters, cx| parameters.changes(cx))
        else {
            return;
        };
        let Some(counts) = self.port_counts(cx) else {
            return;
        };
        let Some(form) = &self.form else {
            return;
        };
        let mut values = form.values.clone();
        values.extend(changes);
        self.query(
            Preparation {
                values,
                counts,
                field: None,
                create: true,
            },
            window,
            cx,
        );
    }
}

impl EventEmitter<CreationEvent> for NodeCreationView {}
