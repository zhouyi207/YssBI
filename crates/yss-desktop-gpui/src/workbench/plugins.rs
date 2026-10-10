use super::Workbench;
use crate::plugins::{OpenNativeView, PluginKey, PluginViewPanel, PluginsEvent};
use gpui::{AppContext, Context, Window, div, prelude::*, px};
use gpui_component::dock::{DockPlacement, panel_handle};

impl Workbench {
    pub(super) fn connect_plugins(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.plugin_subscription = Some(cx.observe(&self.plugins, |_, _, cx| cx.notify()));
        self.subscriptions.push(cx.subscribe_in(
            &self.plugins,
            window,
            |view, _, _: &PluginsEvent, window, cx| {
                if view.project.is_some() && !view.busy && !view.closing {
                    view.present_panel(
                        panel_handle(view.plugins.clone()),
                        DockPlacement::Center,
                        window,
                        cx,
                    );
                }
            },
        ));
    }

    pub(super) fn connect_native_plugin_views(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.subscriptions.push(cx.subscribe_in(
            &self.plugins,
            window,
            |view, _, event: &OpenNativeView, window, cx| {
                view.open_plugin_view(event.key.clone(), event.view.clone(), window, cx);
            },
        ));
    }

    pub(super) fn open_plugin_view(
        &mut self,
        key: PluginKey,
        declaration: yss_plugin_runtime::PluginView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.closing {
            return;
        }
        let view_id = declaration.id.clone();
        let identity = (key.id().to_owned(), view_id.clone());
        let existing = self
            .plugin_views
            .get(&identity)
            .and_then(gpui::WeakEntity::upgrade)
            .filter(|panel| panel.read(cx).matches(&key))
            .or_else(|| {
                let dock = self.dock.read(cx);
                [
                    DockPlacement::Center,
                    DockPlacement::Left,
                    DockPlacement::Right,
                    DockPlacement::Bottom,
                ]
                .into_iter()
                .filter_map(|placement| dock.layout(placement))
                .flat_map(|tree| tree.panels())
                .filter_map(|id| dock.panel(id))
                .filter_map(|panel| panel.view().downcast::<PluginViewPanel>().ok())
                .find(|panel| panel.read(cx).matches_view(&key, &view_id))
            });
        let panel = existing.unwrap_or_else(|| {
            let panel = cx.new(|cx| {
                PluginViewPanel::new(
                    self.services.clone(),
                    key.id().to_owned(),
                    view_id,
                    Some(key),
                    window,
                    cx,
                )
            });
            self.plugin_views.insert(identity, panel.downgrade());
            self.subscribe_plugin_view(&panel, window, cx);
            panel
        });
        if self.project.is_some() {
            let placement = match declaration.location {
                yss_plugin_runtime::ViewLocation::Sidebar => DockPlacement::Left,
                yss_plugin_runtime::ViewLocation::Editor => DockPlacement::Center,
            };
            self.present_panel(panel_handle(panel), placement, window, cx);
        } else {
            crate::modal_window::open(
                declaration.title,
                gpui::size(px(720.), px(640.)),
                window,
                cx,
                move |_, _| {
                    let closed = panel.clone();
                    crate::modal_window::ModalContent::new(move |_, _| panel.clone())
                        .without_buttons()
                        .on_closed(move |cx| closed.update(cx, |panel, _| panel.close()))
                },
            );
        }
    }

    pub(super) fn subscribe_plugin_view(
        &mut self,
        panel: &gpui::Entity<PluginViewPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.subscriptions.push(cx.subscribe_in(
            panel,
            window,
            |view, _, event: &OpenNativeView, window, cx| {
                view.open_plugin_view(event.key.clone(), event.view.clone(), window, cx);
            },
        ));
    }

    pub(super) fn show_plugins(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_closing(cx) {
            return;
        }
        self.plugins.update(cx, |view, cx| view.reload(window, cx));
        if self.project.is_some() {
            self.present_panel(
                panel_handle(self.plugins_sidebar.clone()),
                DockPlacement::Left,
                window,
                cx,
            );
        } else {
            let panel = self.plugins.clone();
            let sidebar = self.plugins_sidebar.clone();
            crate::modal_window::open(
                crate::text::t("activityBar.plugins"),
                gpui::size(px(1020.), px(700.)),
                window,
                cx,
                move |_, _| {
                    let cancel = panel.clone();
                    crate::modal_window::ModalContent::new(move |_, _| {
                        div()
                            .h(px(580.))
                            .flex()
                            .child(div().w(px(260.)).flex_shrink_0().child(sidebar.clone()))
                            .child(div().flex_1().min_w_0().child(panel.clone()))
                    })
                    .without_buttons()
                    .on_cancel(move |_, _, cx| !cancel.read(cx).busy())
                },
            );
        }
    }
}
