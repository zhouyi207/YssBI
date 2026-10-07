use super::{PluginKey, PluginsPanel, commands::PluginAction};
use crate::appearance;
use gpui::{AnyElement, Context, IntoElement, Render, Window, div, prelude::*, px, uniform_list};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui_kit_assets::IconName;
use std::sync::Arc;
use yss_application::activity_panel::{ActivityItem, ActivityRowContent, plugins_activity_panel};

impl Render for PluginsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("native-plugins")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        div()
                            .w(px(260.))
                            .flex_shrink_0()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(cx.theme().border)
                            .child(div().p_3().child(Input::new(&self.search).small()))
                            .child(self.render_directory(cx)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .child(self.render_details(cx)),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .text_color(if self.error.is_some() {
                        cx.theme().danger
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(
                        self.error
                            .clone()
                            .or_else(|| self.task.map(str::to_owned))
                            .or_else(|| self.feedback.clone())
                            .unwrap_or_else(|| format!("{} 个已安装插件", self.entries.len())),
                    ),
            )
    }
}
impl PluginsPanel {
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let busy = self.busy();
        div()
            .flex()
            .items_center()
            .gap_2()
            .p_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(Icon::new(IconName::Puzzle).size_5())
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("插件"),
            )
            .child(
                Button::new("plugins-install")
                    .small()
                    .primary()
                    .label("安装插件包…")
                    .disabled(busy)
                    .on_click(cx.listener(|view, _, window, cx| view.choose_install(window, cx))),
            )
            .child(
                Button::new("plugins-refresh")
                    .small()
                    .ghost()
                    .icon(IconName::RefreshCw)
                    .tooltip("刷新目录与状态")
                    .disabled(busy)
                    .on_click(cx.listener(|view, _, window, cx| view.reload(window, cx))),
            )
            .child(
                Button::new("plugins-collect")
                    .small()
                    .ghost()
                    .label("回收未使用包…")
                    .disabled(busy)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.request_action(None, PluginAction::CollectPackages, window, cx)
                    })),
            )
    }
    fn render_directory(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.search.read(cx).value().to_lowercase();
        let document = plugins_activity_panel(&self.entries);
        let rows: Arc<[_]> = document
            .rows
            .into_iter()
            .filter_map(|row| match row.content {
                ActivityRowContent::Item(ActivityItem::Plugin {
                    id,
                    name,
                    description,
                    publisher,
                    enabled,
                }) if name.to_lowercase().contains(&query)
                    || publisher.to_lowercase().contains(&query) =>
                {
                    self.entries
                        .iter()
                        .find(|plugin| plugin.manifest.id == id)
                        .map(|plugin| {
                            (
                                PluginKey::from_plugin(plugin),
                                name,
                                description,
                                publisher,
                                enabled,
                            )
                        })
                }
                _ => None,
            })
            .collect();
        if rows.is_empty() {
            return appearance::empty_state(
                IconName::Puzzle,
                if self.entries.is_empty() {
                    "尚未安装插件"
                } else {
                    "没有匹配的插件"
                },
                "从签名安装包接入扩展能力",
                cx,
            )
            .into_any_element();
        }
        let generation = self.generation;
        uniform_list(
            "plugin-directory",
            rows.len(),
            cx.processor(move |view, range: std::ops::Range<usize>, _, cx| {
                range
                    .filter_map(|index| rows.get(index))
                    .map(|(key, name, description, publisher, enabled)| {
                        let key = key.clone();
                        let selected = view.selected.as_ref() == Some(&key);
                        div()
                            .id(gpui::SharedString::from(key.id.clone()))
                            .h(px(100.))
                            .px_3()
                            .py_2()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .bg(if selected {
                                cx.theme().accent
                            } else {
                                cx.theme().background
                            })
                            .hover(|style| style.bg(cx.theme().muted))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .truncate()
                                    .child(name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(description.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(format!(
                                        "{} · {}",
                                        publisher,
                                        if *enabled { "已启用" } else { "已停用" }
                                    )),
                            )
                            .on_click(cx.listener(move |view, _, window, cx| {
                                if view.generation == generation {
                                    view.select(key.clone(), window, cx);
                                }
                            }))
                            .into_any_element()
                    })
                    .collect()
            }),
        )
        .flex_1()
        .into_any_element()
    }
}
