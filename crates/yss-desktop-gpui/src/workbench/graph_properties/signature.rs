use super::{GraphProperties, value::type_options};
use crate::workbench::controls;
use gpui::{AnyElement, Context, Entity, IntoElement, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};
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

    fn value(&self, cx: &gpui::App) -> anyhow::Result<FunctionSignature> {
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
                self.error =
                    Some("请输入有效的类型，例如 Numeric、DataSeries<Text> 或 DataFrame。".into());
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
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("函数接口"),
                    )
                    .child(
                        controls::apply("apply-function-signature", busy)
                            .label("应用接口")
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.accepts_input(generation, cx) {
                                    view.apply_signature(cx);
                                }
                            })),
                    ),
            )
            .child(controls::hint(
                "输入参数保持身份，修改接口会更新调用节点",
                cx,
            ))
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
                                    .w(gpui::px(62.))
                                    .flex_shrink_0()
                                    .child(controls::choice(
                                        ("function-parameter-type", row),
                                        "类型".into(),
                                        None,
                                        type_options(),
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
                                    .tooltip("上移参数")
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
                                    .tooltip("下移参数")
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
                                    .tooltip("移除此参数")
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
                    .label("添加输入参数")
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
                                    InputState::new(window, cx).default_value(format!(
                                        "参数{}",
                                        draft.parameters.len() + 1
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
                gpui_component::checkbox::Checkbox::new("signature-has-return")
                    .label("返回值")
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
                                .w(gpui::px(62.))
                                .flex_shrink_0()
                                .child(controls::choice(
                                    "function-return-type",
                                    "类型".into(),
                                    None,
                                    type_options(),
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
