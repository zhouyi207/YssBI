//! Node interactions retain the exact Application catalog descriptor.
use super::*;
use gpui::{AnyElement, Div, Stateful, WeakEntity};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
};
use yss_project_identity::ProjectInstanceId;

#[derive(Clone)]
pub(crate) struct NodeDrag {
    source: WeakEntity<ActivityPanel>,
    document: Arc<ActivityPanelDocument>,
    row: usize,
}

impl NodeDrag {
    pub(crate) fn creation(&self, project: &ProjectInstanceId, cx: &App) -> Option<&NodeCreation> {
        if self.document.project_instance_id.as_deref() != Some(project.as_str())
            || self
                .source
                .upgrade()
                .is_none_or(|source| !Arc::ptr_eq(&source.read(cx).document, &self.document))
        {
            return None;
        }
        match &self.document.rows.get(self.row)?.content {
            ActivityRowContent::Item(ActivityItem::Node {
                available: true,
                creation,
                ..
            }) => Some(creation),
            _ => None,
        }
    }
}

impl Render for NodeDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = div()
            .p_2()
            .max_w(px(280.))
            .rounded_md()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .border_1()
            .border_color(cx.theme().border);
        if let Some(row) = self.document.rows.get(self.row)
            && let ActivityRowContent::Item(ActivityItem::Node {
                title,
                creation,
                available,
                ..
            }) = &row.content
        {
            body.child(crate::catalog_rows::node(title, creation, *available, cx))
        } else {
            body
        }
    }
}

impl ActivityPanel {
    pub(super) fn render_node(
        &self,
        index: usize,
        item: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row = &self.document.rows[index];
        let ActivityRowContent::Item(ActivityItem::Node {
            title,
            available,
            creation,
            ..
        }) = &row.content
        else {
            return item.into_any_element();
        };
        let expected = self.document.clone();
        let expected_creation = expected.clone();
        item.child(crate::catalog_rows::node(title, creation, *available, cx))
            .cursor_pointer()
            .hover(|view| view.bg(cx.theme().muted))
            .on_click(cx.listener(move |view, _, _, cx| {
                if Arc::ptr_eq(&view.document, &expected)
                    && let ActivityRowContent::Item(ActivityItem::Node { creation, .. }) =
                        &view.document.rows[index].content
                {
                    cx.emit(ActivityEvent::InspectNode(
                        crate::catalog_rows::node_type(creation).clone(),
                    ));
                }
            }))
            .when(*available, |item| {
                item.on_drag(
                    NodeDrag {
                        source: cx.entity().downgrade(),
                        document: self.document.clone(),
                        row: index,
                    },
                    |drag, _, _, cx| {
                        cx.stop_propagation();
                        cx.new(|_| drag.clone())
                    },
                )
                .child(
                    Button::new(gpui::SharedString::from(format!("add-{}", row.id)))
                        .small()
                        .ghost()
                        .size_5()
                        .icon(IconName::Plus)
                        .tooltip(crate::text::translate("native.workbench.addToGraph"))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |view, _, _, cx| {
                            cx.stop_propagation();
                            if Arc::ptr_eq(&view.document, &expected_creation)
                                && let ActivityRowContent::Item(ActivityItem::Node {
                                    available: true,
                                    creation,
                                    ..
                                }) = &view.document.rows[index].content
                            {
                                cx.emit(ActivityEvent::CreateNode(creation.clone()));
                            }
                        })),
                )
            })
            .into_any_element()
    }
}
