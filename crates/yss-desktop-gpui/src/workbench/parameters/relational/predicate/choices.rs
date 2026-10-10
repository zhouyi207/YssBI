//! Build typed choices on menu open; revalidate them against the current draft on selection.
use super::{FilterDraft, literal_label, operator_label};
use crate::{
    text::translate,
    workbench::parameters::{ParameterForm, field::ParameterDraft, relational::RelationalDraft},
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Disableable, Sizable,
    button::Button,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{AnyElement, Context, IntoElement, prelude::*};
use yss_graph_editor::projection::{EditorFilterLiteralType, EditorParameterConfiguration};
use yss_node_protocol::dataframe::FilterOperator;

#[derive(Clone, Copy)]
pub(super) enum FilterMenu {
    Column,
    Operator,
    LiteralType,
}

enum Choice {
    Column(String),
    Operator(FilterOperator),
    LiteralType(EditorFilterLiteralType),
}

impl Choice {
    fn presentation(&self, draft: &FilterDraft, cx: &gpui_kit::App) -> (String, bool) {
        match self {
            Self::Column(name) => (name.clone(), draft.column.read(cx).value() == name.as_str()),
            Self::Operator(value) => (operator_label(*value), draft.operator == Some(*value)),
            Self::LiteralType(value) => (literal_label(*value), draft.literal_type == *value),
        }
    }
}

impl ParameterForm {
    pub(super) fn filter_choice(
        &self,
        index: usize,
        kind: FilterMenu,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let field = &self.fields[index];
        let ParameterDraft::Relational(RelationalDraft::Filter(draft)) = &field.draft else {
            unreachable!()
        };
        let configuration = field
            .model
            .configuration
            .as_ref()
            .expect("filter configuration");
        let (id, label, empty) = match kind {
            FilterMenu::Column => {
                let EditorParameterConfiguration::FilterPredicate { columns, .. } = configuration
                else {
                    unreachable!()
                };
                let name = draft.column.read(cx).value();
                (
                    "filter-column",
                    if name.is_empty() {
                        translate("detail.parameterEditor.column")
                    } else {
                        name.to_string()
                    },
                    columns.is_empty(),
                )
            }
            FilterMenu::Operator => (
                "filter-operator",
                draft
                    .operator
                    .map(operator_label)
                    .unwrap_or_else(|| translate("native.workbench.comparison")),
                draft.operators(configuration, cx).is_empty(),
            ),
            FilterMenu::LiteralType => (
                "filter-value-type",
                literal_label(draft.literal_type),
                draft.literal_types(configuration, cx).is_empty(),
            ),
        };
        let epoch = self.epoch;
        let owner = cx.entity().downgrade();
        Button::new((id, index))
            .small()
            .w_full()
            .label(label)
            .icon(IconName::ChevronDown)
            .disabled(busy || empty)
            .dropdown_menu(move |mut menu, _, cx| {
                menu = menu.scrollable(true);
                let Some(view) = owner.upgrade() else {
                    return menu;
                };
                let view = view.read(cx);
                if !view.accepts_input(epoch, cx) {
                    return menu;
                }
                let field = &view.fields[index];
                let ParameterDraft::Relational(RelationalDraft::Filter(draft)) = &field.draft
                else {
                    return menu;
                };
                let Some(configuration) = field.model.configuration.as_ref() else {
                    return menu;
                };
                let column = draft.column.read(cx).value();
                let choices: Vec<_> = match kind {
                    FilterMenu::Column => {
                        let EditorParameterConfiguration::FilterPredicate { columns, .. } =
                            configuration
                        else {
                            return menu;
                        };
                        columns
                            .iter()
                            .map(|column| Choice::Column(column.name.to_string()))
                            .collect()
                    }
                    FilterMenu::Operator => draft
                        .operators(configuration, cx)
                        .iter()
                        .copied()
                        .map(Choice::Operator)
                        .collect(),
                    FilterMenu::LiteralType => draft
                        .literal_types(configuration, cx)
                        .iter()
                        .copied()
                        .map(Choice::LiteralType)
                        .collect(),
                };
                for choice in choices {
                    let (label, checked) = choice.presentation(draft, cx);
                    let owner = owner.clone();
                    let column = column.clone();
                    menu = menu.item(PopupMenuItem::new(label).checked(checked).on_click(
                        move |_, window, cx| {
                            let _ = owner.update(cx, |view, cx| {
                                if !view.accepts_input(epoch, cx) {
                                    return;
                                }
                                let field = &mut view.fields[index];
                                let ParameterDraft::Relational(RelationalDraft::Filter(draft)) =
                                    &mut field.draft
                                else {
                                    return;
                                };
                                let Some(configuration) = field.model.configuration.as_ref() else {
                                    return;
                                };
                                if draft.column.read(cx).value() != column {
                                    return;
                                }
                                let changed = match &choice {
                                    Choice::Column(name) => {
                                        draft.choose_column(name, configuration, window, cx)
                                    }
                                    Choice::Operator(operator) => {
                                        if draft.operator == Some(*operator)
                                            || !draft
                                                .operators(configuration, cx)
                                                .contains(operator)
                                        {
                                            false
                                        } else {
                                            draft.operator = Some(*operator);
                                            true
                                        }
                                    }
                                    Choice::LiteralType(kind) => {
                                        draft.literal_types(configuration, cx).contains(kind)
                                            && draft.choose_type(*kind, window, cx)
                                    }
                                };
                                if changed {
                                    field.error = None;
                                    field.dirty = true;
                                    cx.notify();
                                }
                            });
                        },
                    ));
                }
                menu
            })
            .into_any_element()
    }
}
