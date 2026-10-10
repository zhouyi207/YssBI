//! Shared catalog row presentation; each host owns its interactions.
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon, tooltip::Tooltip};
use gpui_kit::{App, Div, Stateful, div, prelude::*};
use yss_node_catalog::NodeCreation;
use yss_node_protocol::NodeTypeId;

pub(crate) fn node_type(creation: &NodeCreation) -> &NodeTypeId {
    match creation {
        NodeCreation::Static { node_type_id }
        | NodeCreation::ParameterizedStatic { node_type_id, .. }
        | NodeCreation::ResourceBound { node_type_id, .. } => node_type_id,
    }
}

pub(crate) fn node(
    title: &str,
    creation: &NodeCreation,
    available: bool,
    cx: &App,
) -> Stateful<Div> {
    let hint = format!("{title}\n{}", node_type(creation));
    div()
        .id("catalog-item-content")
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap_1p5()
        .tooltip(move |window, cx| Tooltip::new(hint.clone()).build(window, cx))
        .child(
            Icon::new(if matches!(creation, NodeCreation::ResourceBound { .. }) {
                IconName::Braces
            } else {
                IconName::Frame
            })
            .size_3()
            .text_color(cx.theme().muted_foreground),
        )
        .child(div().flex_1().min_w_0().truncate().child(title.to_owned()))
        .when(!available, |view| {
            view.text_color(cx.theme().muted_foreground).child(
                div()
                    .text_xs()
                    .flex_shrink_0()
                    .child(crate::text::translate("canvas.nodePalette.unavailable")),
            )
        })
}

pub(crate) fn category(label: String, expanded: bool, cx: &App) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap_1()
        .child(
            Icon::new(if expanded {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
            .size_3(),
        )
        .child(
            Icon::new(if expanded {
                IconName::FolderOpen
            } else {
                IconName::Folder
            })
            .size_3()
            .text_color(cx.theme().muted_foreground),
        )
        .child(div().flex_1().min_w_0().truncate().child(label))
}
