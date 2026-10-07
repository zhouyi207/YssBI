use super::Workbench;
use gpui::{Context, Window, div, prelude::*, px};
use gpui_component::{
    WindowExt,
    dock::{DockPlacement, panel_handle},
};

impl Workbench {
    pub(super) fn show_plugins(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_closing(cx) {
            return;
        }
        self.plugins.update(cx, |view, cx| view.reload(window, cx));
        if self.project.is_some() {
            self.present_panel(
                panel_handle(self.plugins.clone()),
                DockPlacement::Center,
                window,
                cx,
            );
        } else {
            let panel = self.plugins.clone();
            window.open_dialog(cx, move |dialog, _, _| {
                let cancel = panel.clone();
                dialog
                    .title("插件")
                    .width(px(980.))
                    .overlay_closable(false)
                    .footer(div())
                    .child(div().h(px(580.)).child(panel.clone()))
                    .on_cancel(move |_, _, cx| !cancel.read(cx).busy())
            });
        }
    }
}
