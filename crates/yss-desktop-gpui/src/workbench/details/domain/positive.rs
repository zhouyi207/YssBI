//! Positive selection follows the authored row; menu labels are built on open.
use super::{DetailsPanel, DomainDraft, ParameterDraft};
use crate::text::translate;
use gpui::{AnyElement, Context, IntoElement, div, prelude::*};
use gpui_component::{
    Disableable, Sizable,
    button::Button,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit_assets::IconName;

impl DetailsPanel {
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
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(super::controls::hint(
                translate("conversion.positiveValue"),
                cx,
            ))
            .child(
                Button::new(("domain-positive", index))
                    .small()
                    .w_full()
                    .label(label)
                    .icon(IconName::ChevronDown)
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
                        let options = std::iter::once((
                            None,
                            String::new(),
                            translate("conversion.unspecified"),
                        ))
                        .chain((0..draft.len()).map(|row| {
                            (
                                Some(row),
                                draft.code(row, cx).unwrap_or_default().into_owned(),
                                draft.choice_label(row, cx),
                            )
                        }))
                        .collect::<Vec<_>>();
                        for (row, code, label) in options {
                            let owner = owner.clone();
                            menu = menu.item(
                                PopupMenuItem::new(label)
                                    .checked(draft.positive == row)
                                    .on_click(move |_, _, cx| {
                                        let _ = owner.update(cx, |view, cx| {
                                            if !view.accepts_input(epoch, cx) {
                                                return;
                                            }
                                            let field = &mut view.fields[index];
                                            let ParameterDraft::Domain(draft) = &mut field.draft
                                            else {
                                                return;
                                            };
                                            if row.is_some_and(|row| {
                                                draft.code(row, cx).as_deref()
                                                    != Some(code.as_str())
                                            }) {
                                                return;
                                            }
                                            if draft.positive != row {
                                                draft.positive = row;
                                                field.error = None;
                                                cx.notify();
                                            }
                                        });
                                    }),
                            );
                        }
                        menu
                    }),
            )
            .into_any_element()
    }
}
