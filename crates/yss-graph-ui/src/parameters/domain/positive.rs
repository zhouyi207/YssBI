//! Positive selection follows the authored row; menu labels are built on open.
use super::{DomainDraft, ParameterDraft, ParameterForm};
use gpui_kit::component::{
    Disableable,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{AnyElement, Context, IntoElement, div};
use yss_ui::inspector;
use yss_ui::text::translate;

impl ParameterForm {
    pub(super) fn domain_positive(
        &self,
        index: usize,
        draft: &DomainDraft,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Preserve an already authored positive value when the mapping is resized.
        if draft.len() != 2 && draft.positive.is_none() {
            return div().into_any_element();
        }
        let label = draft
            .positive
            .map(|row| draft.choice_label(row, cx))
            .unwrap_or_else(|| translate("conversion.unspecified"));
        let epoch = self.epoch;
        let owner = cx.entity().downgrade();
        let trigger = inspector::choice_trigger(("domain-positive", index), label)
            .disabled(busy)
            .dropdown_menu(move |mut menu, _, cx| {
                menu = menu.scrollable(true);
                let Some(view) = owner.upgrade() else {
                    return menu;
                };
                let view = view.read(cx);
                if !view.accepts_input(epoch, cx) {
                    return menu;
                }
                let ParameterDraft::Domain(draft) = &view.fields[index].draft else {
                    return menu;
                };
                let options =
                    std::iter::once((None, String::new(), translate("conversion.unspecified")))
                        .chain((0..draft.len()).map(|row| {
                            (
                                Some(row),
                                draft.code(row, cx).unwrap_or_default().into_owned(),
                                draft.choice_label(row, cx),
                            )
                        }))
                        .collect::<Vec<_>>();
                for (row, code, label) in options {
                    menu = menu.item(
                        PopupMenuItem::new(label)
                            .checked(draft.positive == row)
                            .on_click(inspector::owner_command(
                                owner.clone(),
                                move |view: &mut ParameterForm, _, cx| {
                                    if !view.accepts_input(epoch, cx) {
                                        return;
                                    }
                                    let field = &mut view.fields[index];
                                    let ParameterDraft::Domain(draft) = &mut field.draft else {
                                        return;
                                    };
                                    if row.is_some_and(|row| {
                                        draft.code(row, cx).as_deref() != Some(code.as_str())
                                    }) {
                                        return;
                                    }
                                    if draft.positive != row {
                                        draft.positive = row;
                                        field.error = None;
                                        field.dirty = true;
                                        cx.notify();
                                    }
                                },
                            )),
                    );
                }
                menu
            });
        inspector::sub_field(translate("conversion.positiveValue"), trigger, cx).into_any_element()
    }
}
