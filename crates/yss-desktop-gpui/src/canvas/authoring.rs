//! Graph-wide authoring commands keep their original Project transaction owners.
use gpui::Context;
use yss_data_contract::{DataValue, ValueType};
use yss_graph_document::ConstantId;
use yss_graph_editor::EditorGraphMutation;
use yss_project::GraphEditVersion;
use yss_project_history::{FunctionDocumentPatch, MutationRequest};

use super::{GraphCanvas, GraphCommand};

pub enum ConstantValueInput {
    Null,
    Boolean(bool),
    Text(String),
    Numeric(String),
    Json(String),
}

pub(super) fn parse_constant_input(
    value: ConstantValueInput,
    data_type: &ValueType,
) -> anyhow::Result<DataValue> {
    Ok(match value {
        ConstantValueInput::Null => DataValue::Null,
        ConstantValueInput::Boolean(value) => DataValue::Bool(value),
        ConstantValueInput::Text(value) => DataValue::String(value.into()),
        ConstantValueInput::Numeric(value) => {
            crate::constant_values::number(&value).map_err(|_| {
                crate::constant_values::InputError("detail.constantValue.errors.invalidValue")
            })?
        }
        ConstantValueInput::Json(source)
            if matches!(data_type, ValueType::DataFrame | ValueType::DataSeries(_)) =>
        {
            DataValue::String(source.into())
        }
        ConstantValueInput::Json(source) => {
            let value = crate::constant_values::parse_json(&source)?;
            let invalid = match data_type {
                ValueType::Array(_) if !matches!(value, DataValue::List(_)) => {
                    Some("detail.constantValue.errors.notArray")
                }
                ValueType::Object if !matches!(value, DataValue::Object(_)) => {
                    Some("detail.constantValue.errors.notObject")
                }
                _ => None,
            };
            if let Some(key) = invalid {
                return Err(crate::constant_values::InputError(key).into());
            }
            value
        }
    })
}

impl GraphCanvas {
    pub(crate) fn submit_signature(
        &mut self,
        request: MutationRequest<FunctionDocumentPatch>,
        version: GraphEditVersion,
        cx: &mut Context<Self>,
    ) {
        if !self.can_edit() || self.graph.editing.version != version {
            return;
        }
        self.begin_command(cx);
        let project = self.graph.project.clone();
        let path = self.graph.projection.graph_path.clone();
        let publisher = self.services.clone();
        let task = self.services.run(move |services| {
            services
                .application
                .current_graph_document(&project, &path, version)?;
            let result = services
                .application
                .update_function_signature(project, path, request)?;
            publisher.publish_resource(result);
            Ok(())
        });
        cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|value| value);
            let _ = view.update(cx, |view, cx| {
                view.busy = false;
                if result.is_err() {
                    view.error = Some("函数签名未能修改，请刷新属性后重试。".into());
                    tracing::warn!(
                        code = "native_signature_update_failed",
                        "Native signature command failed"
                    );
                }
                view.refresh_pending = true;
                view.refresh(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn insert_constant_reference(
        &mut self,
        id: ConstantId,
        version: GraphEditVersion,
        cx: &mut Context<Self>,
    ) {
        self.submit(
            GraphCommand::Edit(EditorGraphMutation::InsertConstantReference {
                id,
                position: self.world(self.bounds.get().center()),
            }),
            Some(version),
            cx,
        );
    }
}
