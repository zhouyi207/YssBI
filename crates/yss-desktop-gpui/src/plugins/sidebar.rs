//! Sidebar projection of the shared plugin manager; it owns no plugin selection or reads.
use super::{PluginKey, PluginsPanel, commands::PluginAction};
use crate::appearance;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    Subscription, Window, div, prelude::*, px, uniform_list,
};
use std::sync::Arc;
use yss_application::activity_panel::{ActivityItem, ActivityRowContent, plugins_activity_panel};

pub(crate) struct PluginsSidebar {
    owner: Entity<PluginsPanel>,
    focus: FocusHandle,
    _subscription: Subscription,
}

impl PluginsSidebar {
    pub(crate) fn new(owner: Entity<PluginsPanel>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&owner, |_, _, cx| cx.notify());
        Self {
            owner,
            focus: cx.focus_handle(),
            _subscription: subscription,
        }
    }
}

impl EventEmitter<PanelEvent> for PluginsSidebar {}
impl Focusable for PluginsSidebar {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for PluginsSidebar {
    fn panel_name(&self) -> &'static str {
        "plugins-directory"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
    fn zoomable(&self, _: &App) -> bool {
        false
    }
    fn set_active(&mut self, active: bool, window: &mut Window, cx: &mut Context<Self>) {
        if active && self.owner.read(cx).generation == 0 {
            self.owner.update(cx, |view, cx| view.reload(window, cx));
        }
    }
}
impl Panel for PluginsSidebar {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        crate::text::t("activityBar.plugins")
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
impl Render for PluginsSidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let input = self.owner.read(cx).search.clone();
        crate::text::input_placeholder(&input, "native.plugins.search", window, cx);

        div()
            .id("plugins-sidebar")
            .track_focus(&self.focus)
            .size_full()
            .min_w_0()
            .child(self.owner.update(cx, |view, cx| view.render_sidebar(cx)))
    }
}

impl PluginsPanel {
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .size_full()
            .min_w_0()
            .flex()
            .flex_col()
            .bg(cx.theme().sidebar)
            .child(self.render_directory_toolbar(cx))
            .child(div().p_2().child(Input::new(&self.search).small()))
            .child(div().flex_1().min_h_0().child(self.render_directory(cx)))
            .child(self.render_status(cx))
    }

    fn render_directory_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let busy = self.busy();
        let owner = cx.entity().downgrade();
        div()
            .h(px(36.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .child(crate::text::t("activityBar.plugins")),
            )
            .child(
                Button::new("plugins-install")
                    .small()
                    .ghost()
                    .icon(IconName::Plus)
                    .tooltip(crate::text::t("native.plugins.installPackage"))
                    .disabled(busy)
                    .on_click(cx.listener(|view, _, window, cx| view.choose_install(window, cx))),
            )
            .child(
                Button::new("plugins-refresh")
                    .small()
                    .ghost()
                    .icon(IconName::RefreshCw)
                    .tooltip(crate::text::t("native.plugins.refreshCatalog"))
                    .disabled(busy)
                    .on_click(cx.listener(|view, _, window, cx| view.reload(window, cx))),
            )
            .child(
                Button::new("plugins-options")
                    .small()
                    .ghost()
                    .icon(IconName::Ellipsis)
                    .tooltip(crate::text::t("native.plugins.options"))
                    .disabled(busy)
                    .dropdown_menu(move |menu, _, _| {
                        let owner = owner.clone();
                        menu.item(
                            PopupMenuItem::new(crate::text::t("native.plugins.collectPackages"))
                                .on_click(move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        view.request_action(
                                            None,
                                            PluginAction::CollectPackages,
                                            window,
                                            cx,
                                        )
                                    });
                                }),
                        )
                    }),
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
                    crate::text::t("native.plugins.noPlugins")
                } else {
                    crate::text::t("native.plugins.noMatches")
                },
                crate::text::t("native.plugins.installHint"),
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
                            .id(gpui_kit::SharedString::from(key.id.clone()))
                            .h(px(88.))
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
                                    .font_weight(gpui_kit::FontWeight::MEDIUM)
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
                                        if *enabled {
                                            crate::text::t("native.plugins.enabled")
                                        } else {
                                            crate::text::t("native.plugins.disabled")
                                        }
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
