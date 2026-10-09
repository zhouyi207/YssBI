//! Category controls dispatch the tools from the same immutable directory row.
use super::*;
use gpui::{AnyElement, Div, Stateful};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    menu::{ContextMenuExt, PopupMenuItem},
};

impl ActivityPanel {
    pub(super) fn render_category(
        &self,
        index: usize,
        item: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(document) = &self.document else {
            return item.into_any_element();
        };
        let row = &document.rows[index];
        let ActivityRowContent::Category {
            label,
            count,
            tools,
            default_expanded,
        } = &row.content
        else {
            return item.into_any_element();
        };
        let id = row.id.clone();
        let expanded = self.expanded.get(&id).copied().unwrap_or(*default_expanded);
        let expected = document.clone();
        let menu_document = expected.clone();
        let owner = cx.entity().downgrade();
        let item = item
            .aria_expanded(expanded)
            .font_weight(gpui::FontWeight::MEDIUM)
            .cursor_pointer()
            .text_color(cx.theme().muted_foreground)
            .hover(|style| style.bg(cx.theme().muted))
            .child(crate::catalog_rows::category(
                activity_text(label),
                expanded,
                cx,
            ))
            .children(
                count.map(|count| div().text_xs().px_1().rounded_sm().child(count.to_string())),
            )
            .children(tools.iter().map(|tool| {
                let tool_id = tool.id.to_owned();
                let expected = document.clone();
                Button::new(gpui::SharedString::from(format!(
                    "category-tool-{}-{tool_id}",
                    row.id
                )))
                .small()
                .ghost()
                .size_5()
                .icon(feedback::tool_icon(tool.icon))
                .tooltip(activity_text(&tool.label))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _, _, cx| {
                    cx.stop_propagation();
                    if view.accepts(&expected) {
                        cx.emit(ActivityEvent::Tool(tool_id.clone()));
                    }
                }))
            }))
            .on_click(cx.listener(move |view, _, _, cx| {
                if view.accepts(&expected) {
                    view.expanded.insert(id.clone(), !expanded);
                    view.rebuild_rows(cx);
                    cx.notify();
                }
            }));
        if self.panel_id == "project" && !tools.is_empty() {
            item.context_menu(move |mut menu, _, _| {
                let ActivityRowContent::Category { tools, .. } = &menu_document.rows[index].content
                else {
                    return menu;
                };
                for tool in tools {
                    let expected = menu_document.clone();
                    let owner = owner.clone();
                    let tool_id = tool.id.to_owned();
                    menu = menu.item(
                        PopupMenuItem::new(activity_text(&tool.label))
                            .icon(feedback::tool_icon(tool.icon))
                            .on_click(move |_, _, cx| {
                                let _ = owner.update(cx, |view, cx| {
                                    if view.accepts(&expected) {
                                        cx.emit(ActivityEvent::Tool(tool_id.clone()));
                                    }
                                });
                            }),
                    );
                }
                menu
            })
            .into_any_element()
        } else {
            item.into_any_element()
        }
    }
}

impl super::super::Workbench {
    pub(in crate::workbench) fn expand_project_category(&self, id: &str, cx: &mut Context<Self>) {
        if let Some(panel) = self
            .activities
            .get("project")
            .and_then(gpui::WeakEntity::upgrade)
        {
            panel.update(cx, |panel, cx| {
                panel.expanded.insert(id.into(), true);
                panel.rebuild_rows(cx);
                cx.notify();
            });
        }
    }
}
