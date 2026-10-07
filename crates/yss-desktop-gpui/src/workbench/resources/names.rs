//! Name input is a dialog draft; only the captured Project command commits a rename.
use super::{
    super::Workbench,
    operations::{GraphResourceAction, GraphTarget},
};
use gpui::{Context, Window, prelude::*};
use gpui_component::{
    WindowExt,
    dialog::DialogButtonProps,
    input::{Input, InputState},
};

impl Workbench {
    pub(super) fn rename_graph_dialog(
        &mut self,
        target: GraphTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(target.name.clone()));
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let input = input.clone();
            let value = input.clone();
            let target = target.clone();
            let owner = owner.clone();
            dialog
                .title("重命名图")
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("重命名")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .child(Input::new(&input))
                .on_ok(move |_, window, cx| {
                    let name = value.read(cx).value().to_string();
                    if name.trim().is_empty() {
                        return false;
                    }
                    let _ = owner.update(cx, |view, cx| {
                        view.mutate_graph_resource(
                            target.clone(),
                            GraphResourceAction::Rename,
                            Some(name),
                            window,
                            cx,
                        )
                    });
                    true
                })
        });
    }
}
