use super::{GraphProperties, value::type_menu};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Disableable, Sizable,
    checkbox::Checkbox,
    input::{Input, InputState},
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{
    Anchor, AnyElement, Context, ElementId, Entity, IntoElement, Window, div, prelude::*,
};
use yss_data_contract::ValueType;
use yss_graph_document::FunctionParameterId;
use yss_project_history::{
    FunctionDocument, FunctionDocumentPatch, FunctionParameter, FunctionResourceKey,
    FunctionSignature, MutationRequest, ResourceKey,
};
use yss_project_identity::OperationId;
use yss_ui::inspector::{self, InspectorSection};

struct ParameterDraft {
    id: FunctionParameterId,
    name: Entity<InputState>,
    data_type: Entity<InputState>,
}

pub(super) struct SignatureDraft {
    pub baseline: FunctionDocument,
    parameters: Vec<ParameterDraft>,
    return_type: Entity<InputState>,
    has_return: bool,
}

impl SignatureDraft {
    pub fn new(
        baseline: FunctionDocument,
        window: &mut Window,
        cx: &mut Context<GraphProperties>,
    ) -> Self {
        let parameters = baseline
            .signature
            .parameters
            .iter()
            .map(|parameter| ParameterDraft {
                id: parameter.id.clone(),
                name: cx
                    .new(|cx| InputState::new(window, cx).default_value(parameter.name.clone())),
                data_type: cx.new(|cx| {
                    InputState::new(window, cx).default_value(parameter.type_name.clone())
                }),
            })
            .collect();
        let return_type = cx.new(|cx| {
            InputState::new(window, cx).default_value(
                baseline
                    .signature
                    .return_type
                    .clone()
                    .unwrap_or_else(|| "Numeric".into()),
            )
        });
        let has_return = baseline.signature.return_type.is_some();
        Self {
            baseline,
            parameters,
            return_type,
            has_return,
        }
    }

    fn value(&self, cx: &gpui_kit::App) -> anyhow::Result<FunctionSignature> {
        let parameters = self
            .parameters
            .iter()
            .map(|parameter| {
                let data_type: ValueType = parameter.data_type.read(cx).value().parse()?;
                Ok(FunctionParameter {
                    id: parameter.id.clone(),
                    name: parameter.name.read(cx).value().to_string(),
                    type_name: data_type.to_string(),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let return_type = if self.has_return {
            Some(
                self.return_type
                    .read(cx)
                    .value()
                    .parse::<ValueType>()?
                    .to_string(),
            )
        } else {
            None
        };
        Ok(FunctionSignature {
            parameters,
            return_type,
        })
    }
}

impl GraphProperties {
    fn apply_signature(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = &self.signature else {
            return;
        };
        let Some(graph) = self.graph() else {
            return;
        };
        let Some(version) = self.version else {
            return;
        };
        let after = match draft.value(cx) {
            Ok(value) => value,
            Err(_) => {
                self.error = Some(yss_ui::text::t("native.workbench.invalidType").into());
                cx.notify();
                return;
            }
        };
        if after == draft.baseline.signature {
            return;
        }
        let request = MutationRequest::new(
            ResourceKey::Function(FunctionResourceKey(graph.read(cx).path().into())),
            draft.baseline.revision,
            OperationId::new(),
            FunctionDocumentPatch::new(draft.baseline.signature.clone(), after),
        );
        self.error = None;
        graph.update(cx, |graph, cx| graph.submit_signature(request, version, cx));
        cx.notify();
    }

    pub(super) fn render_signature(
        &self,
        draft: &SignatureDraft,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let generation = self.generation;
        let mut section = InspectorSection::new(
            "function-interface",
            yss_ui::text::translate("native.workbench.functionInterface"),
        )
        .description(yss_ui::text::translate(
            "native.workbench.functionInterfaceHint",
        ))
        .tool(
            inspector::action_button(
                "restore-function-signature",
                IconName::RotateCcw,
                yss_ui::text::translate("common.restore"),
            )
            .disabled(busy)
            .on_click(cx.listener(move |view, _, window, cx| {
                if view.accepts_input(generation, cx)
                    && let Some(draft) = &view.signature
                {
                    view.signature = Some(SignatureDraft::new(draft.baseline.clone(), window, cx));
                    view.error = None;
                    cx.notify();
                }
            })),
        );
        if draft.parameters.is_empty() {
            section = section.child(inspector::hint(
                yss_ui::text::translate("detail.pinEditor.noInputs"),
                cx,
            ));
        }
        for (row, parameter) in draft.parameters.iter().enumerate() {
            let choose_id = parameter.id.clone();
            section = section.child(
                inspector::record(cx)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Input::new(&parameter.name)
                                    .small()
                                    .flex_1()
                                    .min_w_0()
                                    .disabled(busy),
                            )
                            .child(self.render_parameter_menu(
                                row,
                                draft.parameters.len(),
                                parameter.id.clone(),
                                busy,
                                cx,
                            )),
                    )
                    .child(type_row(
                        &parameter.data_type,
                        ("function-parameter-type", row),
                        busy,
                        cx,
                        cx.listener(move |view, value: &String, window, cx| {
                            if view.accepts_input(generation, cx)
                                && let Some(draft) = &mut view.signature
                                && let Some(parameter) = draft
                                    .parameters
                                    .iter()
                                    .find(|parameter| parameter.id == choose_id)
                            {
                                parameter.data_type.update(cx, |input, cx| {
                                    input.set_value(value.clone(), window, cx)
                                });
                            }
                        }),
                    )),
            );
        }
        section
            .child(
                div().flex().flex_col().items_start().child(
                    inspector::add_button(
                        "signature-add",
                        yss_ui::text::translate("native.workbench.addInputParameter"),
                    )
                    .disabled(busy)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        if view.accepts_input(generation, cx)
                            && let Some(draft) = &mut view.signature
                        {
                            draft.parameters.push(ParameterDraft {
                                id: FunctionParameterId::new(format!(
                                    "pin-{}",
                                    uuid::Uuid::new_v4()
                                )),
                                name: cx.new(|cx| {
                                    InputState::new(window, cx).default_value(yss_ui::text::format(
                                        "native.workbench.parameterName",
                                        &[("value0", (draft.parameters.len() + 1).to_string())],
                                    ))
                                }),
                                data_type: cx
                                    .new(|cx| InputState::new(window, cx).default_value("Numeric")),
                            });
                            cx.notify();
                        }
                    })),
                ),
            )
            .child(
                Checkbox::new("signature-has-return")
                    .label(yss_ui::text::translate("native.workbench.returnValue"))
                    .checked(draft.has_return)
                    .disabled(busy)
                    .on_click(cx.listener(move |view, value: &bool, _, cx| {
                        if view.accepts_input(generation, cx)
                            && let Some(draft) = &mut view.signature
                        {
                            draft.has_return = *value;
                            cx.notify();
                        }
                    })),
            )
            .when(draft.has_return, |section| {
                section.child(type_row(
                    &draft.return_type,
                    "function-return-type",
                    busy,
                    cx,
                    cx.listener(move |view, value: &String, window, cx| {
                        if view.accepts_input(generation, cx)
                            && let Some(draft) = &view.signature
                        {
                            draft
                                .return_type
                                .update(cx, |input, cx| input.set_value(value.clone(), window, cx));
                        }
                    }),
                ))
            })
            .child(
                div().flex().justify_end().child(
                    inspector::commit_button(
                        "apply-function-signature",
                        yss_ui::text::translate("native.workbench.applyInterface"),
                        busy,
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.accepts_input(generation, cx) {
                            view.apply_signature(cx);
                        }
                    })),
                ),
            )
            .into_any_element()
    }

    fn render_parameter_menu(
        &self,
        row: usize,
        count: usize,
        id: FunctionParameterId,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let generation = self.generation;
        let owner = cx.entity().downgrade();
        inspector::overflow_button(("signature-actions", row))
            .disabled(busy)
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                let reorder = |direction: isize, label: &str, icon: IconName, disabled: bool| {
                    let id = id.clone();
                    PopupMenuItem::new(yss_ui::text::translate(label))
                        .icon(icon)
                        .disabled(disabled)
                        .on_click(inspector::owner_command(
                            owner.clone(),
                            move |view: &mut GraphProperties, _, cx| {
                                if view.accepts_input(generation, cx) {
                                    view.move_parameter(&id, direction, cx);
                                }
                            },
                        ))
                };
                let remove = id.clone();
                menu.item(reorder(
                    -1,
                    "native.workbench.moveParameterUp",
                    IconName::ArrowUp,
                    row == 0,
                ))
                .item(reorder(
                    1,
                    "native.workbench.moveParameterDown",
                    IconName::ArrowDown,
                    row + 1 >= count,
                ))
                .separator()
                .item(
                    PopupMenuItem::new(yss_ui::text::translate("native.workbench.removeParameter"))
                        .icon(IconName::Trash)
                        .on_click(inspector::owner_command(
                            owner.clone(),
                            move |view: &mut GraphProperties, _, cx| {
                                if view.accepts_input(generation, cx)
                                    && let Some(draft) = &mut view.signature
                                {
                                    draft.parameters.retain(|parameter| parameter.id != remove);
                                    cx.notify();
                                }
                            },
                        )),
                )
            })
            .into_any_element()
    }

    fn move_parameter(
        &mut self,
        id: &FunctionParameterId,
        direction: isize,
        cx: &mut Context<Self>,
    ) {
        if let Some(draft) = &mut self.signature
            && let Some(row) = draft
                .parameters
                .iter()
                .position(|parameter| &parameter.id == id)
            && let Some(target) = row.checked_add_signed(direction)
            && target < draft.parameters.len()
        {
            draft.parameters.swap(row, target);
            cx.notify();
        }
    }
}

/// A free-form type name with a menu of the common types beside it.
fn type_row(
    input: &Entity<InputState>,
    id: impl Into<ElementId>,
    busy: bool,
    cx: &Context<GraphProperties>,
    on_choose: impl Fn(&String, &mut Window, &mut gpui_kit::App) + 'static,
) -> gpui_kit::Div {
    let current = input.read(cx).value().to_string();
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(Input::new(input).small().flex_1().min_w_0().disabled(busy))
        .child(type_menu(
            inspector::action_button(
                id,
                IconName::ChevronDown,
                yss_ui::text::translate("detail.fields.type"),
            )
            .disabled(busy),
            Some(current),
            on_choose,
        ))
}
