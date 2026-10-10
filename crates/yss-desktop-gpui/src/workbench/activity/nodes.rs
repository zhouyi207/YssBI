//! Node interactions retain the exact Application catalog descriptor.
use super::*;
use gpui_kit::component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{AnyElement, Div, Stateful};

impl ActivityPanel {
    pub(super) fn render_node(
        &self,
        index: usize,
        item: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(document) = &self.document else {
            return item.into_any_element();
        };
        let row = &document.rows[index];
        let ActivityRowContent::Item(ActivityItem::Node {
            title,
            available,
            creation,
            ..
        }) = &row.content
        else {
            return item.into_any_element();
        };
        let expected = document.clone();
        let expected_creation = expected.clone();
        item.child(crate::catalog_rows::node(title, creation, *available, cx))
            .cursor_pointer()
            .hover(|view| view.bg(cx.theme().muted))
            .on_click(cx.listener(move |view, _, _, cx| {
                if view.accepts(&expected)
                    && let ActivityRowContent::Item(ActivityItem::Node { creation, .. }) =
                        &expected.rows[index].content
                {
                    cx.emit(ActivityEvent::InspectNode(
                        crate::catalog_rows::node_type(creation).clone(),
                    ));
                }
            }))
            .when(*available, |item| {
                item.on_drag(
                    ActivityDrag::new(self, index, cx).expect("available node row"),
                    |drag, _, _, cx| {
                        cx.stop_propagation();
                        cx.new(|_| drag.clone())
                    },
                )
                .child(
                    Button::new(gpui_kit::SharedString::from(format!("add-{}", row.id)))
                        .small()
                        .ghost()
                        .size_5()
                        .icon(IconName::Plus)
                        .tooltip(crate::text::translate("native.workbench.addToGraph"))
                        .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation()
                        })
                        .on_click(cx.listener(move |view, _, _, cx| {
                            cx.stop_propagation();
                            if view.accepts(&expected_creation)
                                && let ActivityRowContent::Item(ActivityItem::Node {
                                    available: true,
                                    creation,
                                    ..
                                }) = &expected_creation.rows[index].content
                            {
                                cx.emit(ActivityEvent::CreateNode(creation.clone()));
                            }
                        })),
                )
            })
            .into_any_element()
    }
}
