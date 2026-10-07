//! Native input drafts consume the existing parameter projection, never a node schema copy.
mod list;
use super::{DetailsPanel, controls, domain::DomainDraft, relational::RelationalDraft};
use crate::workbench::input::TextField;
use gpui::{AnyElement, Context, Entity, IntoElement, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::InputState,
};
use serde_json::Value;
use yss_data_contract::{SemanticType, ValueType};
use yss_graph_editor::projection::{
    EditorParameterConfiguration, EditorParameterModel, ParameterEditorKind,
};

pub(super) struct ParameterField {
    pub model: EditorParameterModel,
    pub draft: ParameterDraft,
    pub error: Option<String>,
}

pub(super) enum ParameterDraft {
    Text {
        input: TextField,
        format: TextFormat,
    },
    Toggle,
    Select {
        options: Vec<String>,
    },
    Constant,
    Domain(DomainDraft),
    List {
        rows: Vec<Entity<InputState>>,
        numeric: bool,
        page: usize,
    },
    Relational(RelationalDraft),
}

#[derive(Clone, Copy)]
pub(super) enum TextFormat {
    String,
    Number,
    Json,
}

impl ParameterField {
    pub fn new(
        model: EditorParameterModel,
        window: &mut Window,
        cx: &mut Context<DetailsPanel>,
    ) -> Self {
        let draft = if let Some(configuration) = &model.configuration {
            match configuration {
                EditorParameterConfiguration::SelectOptions { options } => ParameterDraft::Select {
                    options: options.iter().map(ToString::to_string).collect(),
                },
                _ => ParameterDraft::Relational(RelationalDraft::new(configuration, window, cx)),
            }
        } else if model.editor == ParameterEditorKind::Toggle
            || (model.editor == ParameterEditorKind::Auto
                && model.value_type == Some(ValueType::Scalar(SemanticType::Binary)))
        {
            ParameterDraft::Toggle
        } else if model.editor == ParameterEditorKind::SemanticDomain {
            ParameterDraft::Domain(DomainDraft::new(model.value.as_ref(), window, cx))
        } else if model.editor == ParameterEditorKind::GraphConstant {
            ParameterDraft::Constant
        } else if let Some(ValueType::DataSeries(inner)) = &model.value_type
            && matches!(
                inner.as_ref(),
                ValueType::Scalar(SemanticType::Numeric | SemanticType::Text)
            )
        {
            ParameterDraft::List {
                rows: model
                    .value
                    .as_ref()
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(|value| {
                        cx.new(|cx| InputState::new(window, cx).default_value(display(value)))
                    })
                    .collect(),
                numeric: **inner == ValueType::Scalar(SemanticType::Numeric),
                page: 0,
            }
        } else {
            let format = match model.editor {
                ParameterEditorKind::Text
                | ParameterEditorKind::Resource
                | ParameterEditorKind::Select => TextFormat::String,
                ParameterEditorKind::Number => TextFormat::Number,
                _ if model.value_type == Some(ValueType::Scalar(SemanticType::Text)) => {
                    TextFormat::String
                }
                _ if model.value_type == Some(ValueType::Scalar(SemanticType::Numeric)) => {
                    TextFormat::Number
                }
                _ => TextFormat::Json,
            };
            ParameterDraft::Text {
                input: TextField::new(
                    model
                        .value
                        .as_ref()
                        .map(|value| match format {
                            TextFormat::Json => value.to_string(),
                            _ => display(value),
                        })
                        .unwrap_or_default(),
                    model.multiline,
                    window,
                    cx,
                ),
                format,
            }
        };
        Self {
            model,
            draft,
            error: None,
        }
    }

    pub fn value(&self, cx: &gpui::App) -> Result<Value, String> {
        match &self.draft {
            ParameterDraft::Text { input, format } => {
                let text = input.value(cx);
                match format {
                    TextFormat::String => Ok(Value::String(text.to_string())),
                    TextFormat::Number => controls::number(&text),
                    TextFormat::Json => {
                        serde_json::from_str(&text).map_err(|_| "请输入有效的结构化值".into())
                    }
                }
            }
            ParameterDraft::List { rows, numeric, .. } => rows
                .iter()
                .map(|input| {
                    let text = input.read(cx).value();
                    if *numeric {
                        controls::number(&text)
                    } else {
                        Ok(Value::String(text.to_string()))
                    }
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            ParameterDraft::Relational(RelationalDraft::Columns { manual, .. })
                if matches!(
                    self.model.configuration,
                    Some(EditorParameterConfiguration::ProjectColumns {
                        schema_known: false,
                        ..
                    })
                ) =>
            {
                Ok(Value::Array(
                    manual
                        .iter()
                        .map(|input| Value::String(input.read(cx).value().to_string()))
                        .collect(),
                ))
            }
            ParameterDraft::Relational(draft) => draft.value(cx),
            ParameterDraft::Domain(draft) => draft.value(cx),
            _ => Err("请选择参数值".into()),
        }
    }
}

fn display(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

impl DetailsPanel {
    pub(super) fn render_parameters(&self, busy: bool, cx: &mut Context<Self>) -> AnyElement {
        let Some(node) = self.node() else {
            return div().into_any_element();
        };
        let mut content = div()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .border_b_1()
            .border_color(cx.theme().border);
        if self.fields.is_empty() {
            return content
                .child(controls::hint("此节点无需参数配置", cx))
                .into_any_element();
        }
        for group in &node.parameter_groups {
            let fields = group
                .parameters
                .iter()
                .filter_map(|parameter| {
                    self.fields
                        .iter()
                        .position(|field| field.model.key == parameter.key)
                })
                .map(|index| self.render_parameter(index, busy, cx))
                .collect::<Vec<_>>();
            content = content.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child(group.display.title.to_string()),
                    )
                    .children(
                        group
                            .display
                            .description
                            .as_deref()
                            .map(|text| controls::hint(text, cx)),
                    )
                    .children(fields),
            );
        }
        content.into_any_element()
    }

    fn render_parameter(&self, index: usize, busy: bool, cx: &mut Context<Self>) -> AnyElement {
        let field = &self.fields[index];
        let epoch = self.epoch;
        let editor = match &field.draft {
            ParameterDraft::Toggle => Checkbox::new(("parameter-toggle", index))
                .label("启用")
                .checked(field.model.value == Some(Value::Bool(true)))
                .disabled(busy)
                .on_click(cx.listener(move |view, value: &bool, _, cx| {
                    if view.accepts_input(epoch, cx) {
                        view.commit_parameter(index, Value::Bool(*value), cx)
                    }
                }))
                .into_any_element(),
            ParameterDraft::Select { options } => {
                let current = field
                    .model
                    .value
                    .as_ref()
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let unavailable = current
                    .as_ref()
                    .is_some_and(|value| !options.contains(value));
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(controls::choice(
                        ("parameter-select", index),
                        current.clone().unwrap_or_else(|| "请选择".into()),
                        current,
                        options
                            .iter()
                            .map(|value| (value.clone(), value.clone()))
                            .collect(),
                        busy,
                        cx.listener(move |view, value: &String, _, cx| {
                            if view.accepts_input(epoch, cx) {
                                view.commit_parameter(index, Value::String(value.clone()), cx)
                            }
                        }),
                    ))
                    .when(unavailable, |view| {
                        view.child(controls::hint("当前选项已不可用，请重新选择", cx))
                    })
                    .when(options.is_empty(), |view| {
                        view.child(controls::hint("暂无可选项", cx))
                    })
                    .into_any_element()
            }
            ParameterDraft::Constant => {
                let properties = self.properties.read(cx);
                let constants = properties.names();
                let loading = properties.loading();
                let current = field
                    .model
                    .value
                    .as_ref()
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let label = current
                    .as_ref()
                    .and_then(|id| constants.iter().find(|(key, _)| key == id))
                    .map(|(_, label)| label.clone())
                    .unwrap_or_else(|| {
                        if current.is_some() {
                            "常量不可用".into()
                        } else {
                            "选择图常量".into()
                        }
                    });
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(controls::choice(
                        ("parameter-constant", index),
                        label,
                        current,
                        constants.clone(),
                        busy || loading,
                        cx.listener(move |view, value: &String, _, cx| {
                            if view.accepts_input(epoch, cx) {
                                view.commit_parameter(index, Value::String(value.clone()), cx)
                            }
                        }),
                    ))
                    .when(constants.is_empty() && !loading, |view| {
                        view.child(controls::hint("此图尚无常量", cx))
                    })
                    .into_any_element()
            }
            ParameterDraft::Relational(draft) => self.render_relational(index, draft, busy, cx),
            ParameterDraft::Domain(draft) => self.render_domain(index, draft, busy, cx),
            ParameterDraft::List { rows, page, .. } => {
                self.render_list_parameter(index, rows, page, busy, cx)
            }
            ParameterDraft::Text { input, .. } => div()
                .flex()
                .items_start()
                .gap_1()
                .child(input.render(busy))
                .child(
                    controls::apply(("parameter-apply", index), busy).on_click(cx.listener(
                        move |view, _, _, cx| {
                            if view.accepts_input(epoch, cx) {
                                view.apply_parameter(index, cx)
                            }
                        },
                    )),
                )
                .into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .child(field.model.display.title.to_string()),
                    )
                    .child(
                        Button::new(("parameter-reset", index))
                            .small()
                            .ghost()
                            .icon(IconName::Undo2)
                            .tooltip("恢复默认值或清除参数")
                            .disabled(busy)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if view.accepts_input(epoch, cx) {
                                    view.commit_parameter(index, Value::Null, cx)
                                }
                            })),
                    ),
            )
            .children(
                field
                    .model
                    .display
                    .description
                    .as_deref()
                    .map(|text| controls::hint(text, cx)),
            )
            .child(editor)
            .children(field.error.as_ref().map(|error| {
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }))
            .into_any_element()
    }
}
