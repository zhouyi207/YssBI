//! Bounded series pages reuse the same draft for input, reordering and submission.
mod draft;
use super::ParameterForm;
pub(in crate::parameters) use draft::ListDraft;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{
    Anchor, AnyElement, App, Context, Div, ElementId, IntoElement, Window, div, prelude::*,
};
use std::rc::Rc;
use yss_ui::inspector::{self, Pager};
use yss_ui::text::translate;

/// A command from a reorderable draft row's overflow menu.
#[derive(Clone, Copy)]
pub(in crate::parameters) enum RowCommand {
    /// Swap with the neighboring row: -1 moves up, 1 moves down.
    Move(isize),
    Remove,
}

/// One-based position in a draft list; header rows reserve the same `w_6` lane.
pub(in crate::parameters) fn row_number(row: usize, cx: &App) -> Div {
    div()
        .min_w_6()
        .flex_shrink_0()
        .text_right()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child((row + 1).to_string())
}

impl ParameterForm {
    /// Secondary row commands behind one trailing trigger, instead of a strip of icon buttons.
    pub(in crate::parameters) fn row_menu(
        &self,
        id: impl Into<ElementId>,
        (row, len): (usize, usize),
        removable: bool,
        busy: bool,
        cx: &mut Context<Self>,
        apply: impl Fn(&mut Self, RowCommand, &mut Window, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        let owner = cx.entity().downgrade();
        let epoch = self.epoch;
        let apply = Rc::new(apply);
        inspector::overflow_button(id)
            .disabled(busy)
            .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                let item = |command: RowCommand, label: &str, icon: IconName, disabled: bool| {
                    let apply = apply.clone();
                    PopupMenuItem::new(translate(label))
                        .icon(icon)
                        .disabled(disabled)
                        .on_click(inspector::owner_command(
                            owner.clone(),
                            move |view: &mut Self, window, cx| {
                                if view.accepts_input(epoch, cx) {
                                    apply(view, command, window, cx);
                                }
                            },
                        ))
                };
                menu = menu
                    .item(item(
                        RowCommand::Move(-1),
                        "conversion.moveUp",
                        IconName::ArrowUp,
                        row == 0,
                    ))
                    .item(item(
                        RowCommand::Move(1),
                        "conversion.moveDown",
                        IconName::ArrowDown,
                        row + 1 >= len,
                    ));
                if removable {
                    menu = menu.separator().item(item(
                        RowCommand::Remove,
                        "native.workbench.remove",
                        IconName::Trash,
                        false,
                    ));
                }
                menu
            })
            .into_any_element()
    }

    pub(in crate::parameters) fn render_list_parameter(
        &self,
        index: usize,
        draft: &ListDraft,
        busy: bool,
        add_label: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let epoch = self.epoch;
        let len = draft.len();
        let mut values = div()
            .id(("series-values", index))
            .max_h_64()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        for (row, input) in draft.visible() {
            values = values.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(row_number(row, cx))
                    .child(Input::new(input).small().flex_1().min_w_0().disabled(busy))
                    .child(self.row_menu(
                        ("series-actions", input.entity_id().as_u64() as usize),
                        (row, len),
                        true,
                        busy,
                        cx,
                        move |view, command, window, cx| {
                            let Some(field) = view.fields.get_mut(index) else {
                                return;
                            };
                            let Some(draft) = field.list_mut() else {
                                return;
                            };
                            match command {
                                RowCommand::Move(direction) => {
                                    draft.move_row(row, direction, window, cx)
                                }
                                RowCommand::Remove => draft.remove(row, window, cx),
                            }
                            field.error = None;
                            field.dirty = true;
                            cx.notify();
                        },
                    )),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .when(len > 0, |list| list.child(values))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .child(
                        inspector::add_button(("list-add", index), translate(add_label))
                            .disabled(busy)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                if view.accepts_input(epoch, cx)
                                    && let Some(draft) = view.fields[index].list_mut()
                                {
                                    draft.add(window, cx);
                                    view.fields[index].error = None;
                                    view.fields[index].dirty = true;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Pager::new(("list-pager", index), draft.page, draft.pages())
                            .disabled(busy)
                            .on_change(cx.listener(move |view, page: &usize, window, cx| {
                                if view.accepts_input(epoch, cx)
                                    && let Some(draft) = view.fields[index].list_mut()
                                {
                                    draft.show_page(*page, window, cx);
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        inspector::commit_button(
                            ("list-apply", index),
                            translate("native.workbench.applyChanges"),
                            busy,
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if view.accepts_input(epoch, cx) {
                                view.apply_parameter(index, cx);
                            }
                        })),
                    ),
            )
            .into_any_element()
    }
}
