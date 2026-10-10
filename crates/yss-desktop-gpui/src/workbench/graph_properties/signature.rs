use super::{GraphProperties, value::type_picker};
use crate::workbench::controls;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};
use gpui_kit::{AnyElement, Context, Entity, IntoElement, Window, div, prelude::*};
use yss_data_contract::ValueType;
use yss_graph_document::FunctionParameterId;
use yss_project_history::{
    FunctionDocument, FunctionDocumentPatch, FunctionParameter, FunctionResourceKey,
    FunctionSignature, MutationRequest, ResourceKey,
};
use yss_project_identity::OperationId;

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
                self.error = Some(crate::text::t("native.workbench.invalidType").into());
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
        div()
            .flex()
            .flex_col()
            .gap_3()
            .pb_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(crate::text::t("native.workbench.functionInterface")),
                    )
                    .child(
                        Button::new("restore-function-signature")
                            .small()
                            .ghost()
                            .icon(IconName::Undo2)
                            .tooltip(crate::text::translate("common.restore"))
                            .disabled(busy)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                if view.accepts_input(generation, cx)
                                    && let Some(draft) = &view.signature
                                {
                                    view.signature = Some(SignatureDraft::new(
                                        draft.baseline.clone(),
                                        window,
                                        cx,
                                    ));
                                    view.error = None;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        controls::apply("apply-function-signature", busy)
                            .label(crate::text::t("native.workbench.applyInterface"))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.accepts_input(generation, cx) {
                                    view.apply_signature(cx);
                                }
                            })),
                    ),
            )
            .child(controls::hint(
                crate::text::t("native.workbench.functionInterfaceHint"),
                cx,
            ))
            .when(draft.parameters.is_empty(), |view| {
                view.child(controls::hint(
                    crate::text::translate("detail.pinEditor.noInputs"),
                    cx,
                ))
            })
            .children(draft.parameters.iter().enumerate().map(|(row, parameter)| {
                let id = parameter.id.clone();
                let choose_id = id.clone();
                let remove_id = id.clone();
                let up_id = id.clone();
                let down_id = id;
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_2()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .child(Input::new(&parameter.name).small().disabled(busy))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Input::new(&parameter.data_type)
                                    .small()
                                    .flex_1()
                                    .min_w_0()
                                    .disabled(busy),
                            )
                            .child(
                                div()
                                    .w(gpui_kit::px(62.))
                                    .flex_shrink_0()
                                    .child(type_picker(
                                        ("function-parameter-type", row),
                                        crate::text::t("detail.fields.type").into(),
                                        Some(parameter.data_type.read(cx).value().to_string()),
                                        busy,
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
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new(("signature-up", row))
                                    .small()
                                    .ghost()
                                    .icon(IconName::ChevronUp)
                                    .tooltip(crate::text::t("native.workbench.moveParameterUp"))
                                    .disabled(busy || row == 0)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if view.accepts_input(generation, cx) {
                                            view.move_parameter(&up_id, -1, cx);
                                        }
                                    })),
                            )
                            .child(
                                Button::new(("signature-down", row))
                                    .small()
                                    .ghost()
                                    .icon(IconName::ChevronDown)
                                    .tooltip(crate::text::t("native.workbench.moveParameterDown"))
                                    .disabled(busy || row + 1 == draft.parameters.len())
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if view.accepts_input(generation, cx) {
                                            view.move_parameter(&down_id, 1, cx);
                                        }
                                    })),
                            )
                            .child(
                                Button::new(("signature-remove", row))
                                    .small()
                                    .ghost()
                                    .icon(IconName::Close)
                                    .tooltip(crate::text::t("native.workbench.removeParameter"))
                                    .disabled(busy)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if view.accepts_input(generation, cx)
                                            && let Some(draft) = &mut view.signature
                                        {
                                            draft
                                                .parameters
                                                .retain(|parameter| parameter.id != remove_id);
                                            cx.notify();
                                        }
                                    })),
                            ),
                    )
            }))
            .child(
                Button::new("signature-add")
                    .small()
                    .ghost()
                    .icon(IconName::Plus)
                    .label(crate::text::t("native.workbench.addInputParameter"))
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
                                    InputState::new(window, cx).default_value(crate::text::format(
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
            )
            .child(
                gpui_kit::component::checkbox::Checkbox::new("signature-has-return")
                    .label(gpui_kit::SharedString::from(crate::text::t(
                        "native.workbench.returnValue",
                    )))
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
            .when(draft.has_return, |view| {
                view.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            Input::new(&draft.return_type)
                                .small()
                                .flex_1()
                                .min_w_0()
                                .disabled(busy),
                        )
                        .child(
                            div()
                                .w(gpui_kit::px(62.))
                                .flex_shrink_0()
                                .child(type_picker(
                                    "function-return-type",
                                    crate::text::t("detail.fields.type").into(),
                                    Some(draft.return_type.read(cx).value().to_string()),
                                    busy,
                                    cx.listener(move |view, value: &String, window, cx| {
                                        if view.accepts_input(generation, cx)
                                            && let Some(draft) = &view.signature
                                        {
                                            draft.return_type.update(cx, |input, cx| {
                                                input.set_value(value.clone(), window, cx)
                                            });
                                        }
                                    }),
                                )),
                        ),
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
