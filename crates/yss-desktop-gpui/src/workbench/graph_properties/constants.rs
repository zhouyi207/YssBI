mod view;
use super::{GraphProperties, value};
use crate::canvas::{ConstantValueInput, GraphCommand};
use gpui::{AppContext, Context, Entity, Window};
use gpui_component::input::InputState;
use yss_data_contract::{DataValue, SemanticType, ValueType};
use yss_graph_document::{ConstantId, GraphConstant};
use yss_graph_editor::EditorGraphMutation;

#[derive(Clone, PartialEq)]
pub(super) struct ConstantOverview {
    pub id: ConstantId,
    pub name: String,
    data_type: ValueType,
    scalar: Option<String>,
    is_null: bool,
    summary: String,
    // Display-cache equality only; mutation authorization still uses GraphEditVersion.
    fingerprint: [u8; 32],
}

impl ConstantOverview {
    pub fn from_constant(constant: &GraphConstant) -> anyhow::Result<Self> {
        let tabular = matches!(
            constant.data_type,
            ValueType::DataFrame | ValueType::DataSeries(_)
        );
        let scalar = if tabular {
            None
        } else {
            value::scalar_text(&constant.data_value)
        };
        let summary = if let Some(snapshot) = &constant.tabular {
            format!(
                "{} 列 · {} 行",
                snapshot.columns().len(),
                snapshot.row_count()
            )
        } else {
            match &constant.data_value {
                DataValue::Null => "空值".into(),
                DataValue::List(values) => format!("{} 个元素", values.len()),
                DataValue::Object(values) => format!("{} 个字段", values.len()),
                DataValue::String(value) if scalar.is_none() => format!("{} 字节文本", value.len()),
                _ => scalar.clone().unwrap_or_else(|| "结构化值".into()),
            }
        };
        Ok(Self {
            id: constant.id,
            name: constant.name.clone(),
            data_type: constant.data_type.clone(),
            scalar,
            is_null: constant.data_value == DataValue::Null && constant.tabular.is_none(),
            summary,
            fingerprint: yss_canonical_hash::hash_canonical("yssbi.native.constant", constant)?,
        })
    }
}

pub(super) struct ConstantDraft {
    pub model: ConstantOverview,
    name: Entity<InputState>,
    data_type: ValueType,
    is_null: bool,
    input: Option<crate::workbench::input::TextField>,
    original_input: Option<String>,
    value_changed_type: bool,
    pub value_loading: bool,
    pub load_token: u64,
}

fn input_state(
    text: String,
    data_type: &ValueType,
    window: &mut Window,
    cx: &mut Context<GraphProperties>,
) -> crate::workbench::input::TextField {
    let multiline = !matches!(data_type, ValueType::Scalar(_));
    crate::workbench::input::TextField::new(text, multiline, window, cx)
}

impl ConstantDraft {
    pub fn new(
        model: ConstantOverview,
        window: &mut Window,
        cx: &mut Context<GraphProperties>,
    ) -> Self {
        Self {
            name: cx.new(|cx| InputState::new(window, cx).default_value(model.name.clone())),
            data_type: model.data_type.clone(),
            is_null: model.is_null,
            input: model
                .scalar
                .as_ref()
                .map(|text| input_state(text.clone(), &model.data_type, window, cx)),
            original_input: model.scalar.clone(),
            value_changed_type: false,
            value_loading: false,
            load_token: 0,
            model,
        }
    }

    fn value_input(&self, cx: &gpui::App) -> Option<ConstantValueInput> {
        if self.is_null {
            return (!self.model.is_null || self.value_changed_type)
                .then_some(ConstantValueInput::Null);
        }
        let source = self.input.as_ref()?.value(cx).to_string();
        if !self.value_changed_type
            && !self.model.is_null
            && self.original_input.as_ref() == Some(&source)
        {
            return None;
        }
        Some(match self.data_type {
            ValueType::Scalar(SemanticType::Numeric) => ConstantValueInput::Numeric(source),
            ValueType::Scalar(SemanticType::Binary) => {
                ConstantValueInput::Boolean(source == "true")
            }
            ValueType::Scalar(_) => ConstantValueInput::Text(source),
            _ => ConstantValueInput::Json(source),
        })
    }
}

impl GraphProperties {
    fn add_constant(&mut self, cx: &mut Context<Self>) {
        let Some(graph) = self.graph() else {
            return;
        };
        let Some(version) = self.version else {
            return;
        };
        let names = self.names();
        let name = (1..)
            .map(|number| format!("常量{number}"))
            .find(|name| names.iter().all(|(_, existing)| existing != name))
            .expect("finite constants list");
        let id = ConstantId::new();
        let constant = GraphConstant {
            id,
            name,
            data_type: ValueType::number(),
            data_value: DataValue::Integer(0),
            tabular: None,
            description: String::new(),
            tags: vec![],
        };
        graph.update(cx, |graph, cx| {
            graph.submit(
                GraphCommand::Edit(EditorGraphMutation::SetConstant {
                    id,
                    constant: Some(constant),
                }),
                Some(version),
                cx,
            )
        });
    }

    fn apply_constant(&mut self, id: ConstantId, cx: &mut Context<Self>) {
        let Some(graph) = self.graph() else {
            return;
        };
        let Some(version) = self.version else {
            return;
        };
        let Some(field) = self.constants.iter().find(|field| field.model.id == id) else {
            return;
        };
        let name = field.name.read(cx).value().to_string();
        let value = field.value_input(cx);
        if name == field.model.name && field.data_type == field.model.data_type && value.is_none() {
            return;
        }
        let command = GraphCommand::UpdateConstant {
            id,
            name,
            data_type: field.data_type.clone(),
            value,
        };
        graph.update(cx, |graph, cx| graph.submit(command, Some(version), cx));
        cx.notify();
    }

    fn load_constant_value(&mut self, id: ConstantId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(graph) = self.graph() else {
            return;
        };
        let Some(version) = self.version else {
            return;
        };
        let Some(field) = self.constants.iter_mut().find(|field| field.model.id == id) else {
            return;
        };
        field.value_loading = true;
        field.load_token = field.load_token.wrapping_add(1);
        let token = field.load_token;
        let generation = self.generation;
        let graph = graph.read(cx);
        let project = graph.graph.project.clone();
        let path = graph.graph.projection.graph_path.clone();
        let task = self.services.run(move |services| {
            let document = services
                .application
                .current_graph_document(&project, &path, version)?;
            let constant = document
                .constants
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("constant removed"))?;
            value::structured_text(constant)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.generation != generation
                    || view
                        .graph()
                        .is_none_or(|graph| graph.read(cx).graph.editing.version != version)
                {
                    return;
                }
                let Some(field) = view
                    .constants
                    .iter_mut()
                    .find(|field| field.model.id == id && field.load_token == token)
                else {
                    return;
                };
                field.value_loading = false;
                match result {
                    Ok(source) => {
                        field.original_input = Some(source.clone());
                        field.input = Some(input_state(source, &field.data_type, window, cx));
                    }
                    Err(_) => view.error = Some("无法读取常量值，请刷新后重试。".into()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
