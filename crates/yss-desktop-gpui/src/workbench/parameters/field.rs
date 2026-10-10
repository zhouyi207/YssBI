//! Native input drafts consume the existing parameter projection, never a node schema copy.
mod choices;
pub(super) mod list;
mod render;
use super::{ParameterForm, controls, domain::DomainDraft, relational::RelationalDraft};
use crate::workbench::input::TextField;
use gpui_kit::component::input::InputEvent;
use gpui_kit::{
    AnyElement, Context, Entity, EntityId, EventEmitter, IntoElement, Subscription, Window, div,
    prelude::*,
};
use list::ListDraft;
use serde_json::Value;
use yss_data_contract::{SemanticType, ValueType};
use yss_graph_editor::projection::{
    EditorParameterConfiguration, EditorParameterModel, ParameterEditorKind,
};
use yss_node_protocol::ParameterKey;

pub(super) struct ParameterField {
    pub model: EditorParameterModel,
    pub draft: ParameterDraft,
    pub error: Option<String>,
    pub dirty: bool,
    _input_subscription: Option<Subscription>,
}

pub(super) enum ParameterDraft {
    Text {
        input: TextField,
        format: TextFormat,
    },
    Toggle,
    Select,
    Constant,
    Domain(DomainDraft),
    List(ListDraft),
    Relational(RelationalDraft),
}

#[derive(Clone, Copy)]
pub(super) enum TextFormat {
    String,
    Number,
    Json,
}

impl ParameterField {
    pub(super) fn accepts_projection(&self, next: &EditorParameterModel) -> bool {
        let current = &self.model;
        current.key == next.key
            && current.editor == next.editor
            && current.presentation == next.presentation
            && current.value_type == next.value_type
            && current.multiline == next.multiline
            && current.value == next.value
            && match (&current.configuration, &next.configuration) {
                (
                    Some(EditorParameterConfiguration::ProjectColumns {
                        allow_empty,
                        schema_known,
                        options,
                        value,
                        ..
                    }),
                    Some(EditorParameterConfiguration::ProjectColumns {
                        allow_empty: next_allow,
                        schema_known: next_schema,
                        options: next_options,
                        value: next_value,
                        ..
                    }),
                ) => {
                    allow_empty == next_allow
                        && schema_known == next_schema
                        && options == next_options
                        && value == next_value
                }
                (
                    Some(EditorParameterConfiguration::FilterPredicate {
                        schema_known,
                        columns,
                        value,
                        ..
                    }),
                    Some(EditorParameterConfiguration::FilterPredicate {
                        schema_known: next_schema,
                        columns: next_columns,
                        value: next_value,
                        ..
                    }),
                ) => schema_known == next_schema && columns == next_columns && value == next_value,
                (current, next) => current == next,
            }
    }

    pub fn new(
        model: EditorParameterModel,
        window: &mut Window,
        cx: &mut Context<ParameterForm>,
    ) -> Self {
        let draft = if let Some(configuration) = &model.configuration {
            match configuration {
                EditorParameterConfiguration::SelectOptions { .. } => ParameterDraft::Select,
                _ => ParameterDraft::Relational(RelationalDraft::new(&model, window, cx)),
            }
        } else if model.editor == ParameterEditorKind::Toggle
            || (model.editor == ParameterEditorKind::Auto
                && model.value_type == Some(ValueType::Scalar(SemanticType::Binary)))
        {
            ParameterDraft::Toggle
        } else if model.editor == ParameterEditorKind::SemanticDomain {
            ParameterDraft::Domain(DomainDraft::new(&model, window, cx))
        } else if model.editor == ParameterEditorKind::GraphConstant {
            ParameterDraft::Constant
        } else if let Some(ValueType::DataSeries(inner)) = &model.value_type
            && matches!(
                inner.as_ref(),
                ValueType::Scalar(SemanticType::Numeric | SemanticType::Text)
            )
        {
            ParameterDraft::List(ListDraft::new(&model, window, cx))
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
        let input_subscription = match &draft {
            ParameterDraft::Text {
                input: TextField::Single(input),
                ..
            } => Some(subscribe_input(input, model.key.clone(), window, cx)),
            ParameterDraft::Text {
                input: TextField::Multiline(input),
                ..
            } => Some(subscribe_input(input, model.key.clone(), window, cx)),
            _ => None,
        };
        Self {
            model,
            draft,
            error: None,
            dirty: false,
            _input_subscription: input_subscription,
        }
    }

    pub fn value(&self, cx: &gpui_kit::App) -> Result<Value, String> {
        match &self.draft {
            ParameterDraft::Text { input, format } => {
                let text = input.value(cx);
                match format {
                    TextFormat::String => Ok(Value::String(text.to_string())),
                    TextFormat::Number
                        if self.model.value_type
                            == Some(ValueType::Scalar(SemanticType::Numeric)) =>
                    {
                        controls::number(&text)
                    }
                    TextFormat::Number => Err(crate::text::translate(
                        "notifications.parameter.unsupportedNumericType",
                    )),
                    TextFormat::Json => serde_json::from_str(&text).map_err(|_| {
                        crate::text::translate("native.workbench.invalidStructuredValue")
                    }),
                }
            }
            ParameterDraft::List(draft) => draft.value(cx),
            ParameterDraft::Relational(draft) => draft.value(
                self.model
                    .configuration
                    .as_ref()
                    .expect("relational configuration"),
                cx,
            ),
            ParameterDraft::Domain(draft) => draft.value(cx),
            _ => Err(crate::text::translate("native.workbench.chooseParameter")),
        }
    }
}

fn display(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

pub(super) fn subscribe_input<T: EventEmitter<InputEvent> + 'static>(
    input: &Entity<T>,
    key: ParameterKey,
    window: &mut Window,
    cx: &mut Context<ParameterForm>,
) -> Subscription {
    let input_id = input.entity_id();
    cx.subscribe_in(input, window, move |view, _, event, _, cx| {
        if !view.accepts_input(view.epoch, cx) {
            return;
        }
        let Some(index) = view
            .fields
            .iter()
            .position(|field| field.model.key == key && field.owns_input(input_id))
        else {
            return;
        };
        match event {
            InputEvent::PressEnter { .. }
                if !matches!(
                    view.fields[index].draft,
                    ParameterDraft::Text {
                        input: TextField::Multiline(_),
                        ..
                    }
                ) =>
            {
                view.apply_parameter(index, cx)
            }
            InputEvent::Change => {
                let field = &mut view.fields[index];
                field.dirty = true;
                let mut refresh = field.error.take().is_some();
                if let ParameterDraft::Domain(draft) = &mut field.draft {
                    draft.input_changed(input_id);
                    refresh = true;
                }
                if refresh {
                    cx.notify();
                }
            }
            _ => {}
        }
    })
}

impl ParameterField {
    fn list_mut(&mut self) -> Option<&mut ListDraft> {
        match &mut self.draft {
            ParameterDraft::List(draft) => Some(draft),
            ParameterDraft::Relational(draft) => draft.list_mut(),
            _ => None,
        }
    }

    fn owns_input(&self, id: EntityId) -> bool {
        match &self.draft {
            ParameterDraft::Text {
                input: TextField::Single(input),
                ..
            } => input.entity_id() == id,
            ParameterDraft::Text {
                input: TextField::Multiline(input),
                ..
            } => input.entity_id() == id,
            ParameterDraft::List(draft) => draft.owns_input(id),
            ParameterDraft::Domain(draft) => draft.owns_input(id),
            ParameterDraft::Relational(draft) => draft.owns_input(id),
            _ => false,
        }
    }
}

impl ParameterForm {
    pub(super) fn restore_parameter(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let model = self.fields[index].model.clone();
        if let ParameterDraft::Text { input, format } = &self.fields[index].draft {
            let value = model
                .value
                .as_ref()
                .map(|value| match format {
                    TextFormat::Json => value.to_string(),
                    _ => display(value),
                })
                .unwrap_or_default();
            input.set_value(value, window, cx);
            self.fields[index].error = None;
            self.fields[index].dirty = false;
        } else {
            self.fields[index] = ParameterField::new(model, window, cx);
        }
        cx.notify();
    }
}
