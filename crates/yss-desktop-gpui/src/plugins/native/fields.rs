use super::PluginViewPanel;
use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::{App, AppContext, Context, Entity, Window};
use serde_json::{Map, Value};
use yss_plugin_runtime::{NativeField, NativeInput, NativeView, PluginFailure};

pub(super) struct Field {
    pub model: NativeField,
    pub draft: Draft,
}

pub(super) enum Draft {
    Input(Entity<InputState>),
    Multiline(Entity<TextareaState>),
    Boolean(bool),
    Choice(String),
}

impl PluginViewPanel {
    pub(super) fn install_form(
        &mut self,
        model: NativeView,
        saved: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), PluginFailure> {
        let invalid = || PluginFailure::new("plugin_state_invalid");
        if !saved.is_null() && !saved.is_object() {
            return Err(invalid());
        }
        let mut fields = Vec::with_capacity(model.fields.len());
        let mut subscriptions = vec![];
        for field in &model.fields {
            let value = saved
                .get(&field.id)
                .cloned()
                .unwrap_or_else(|| field.input.value());
            if !field.input.accepts(&value) {
                return Err(invalid());
            }
            let draft = match &field.input {
                NativeInput::Boolean { .. } => Draft::Boolean(value.as_bool().ok_or_else(invalid)?),
                NativeInput::Choice { .. } => {
                    Draft::Choice(value.as_str().ok_or_else(invalid)?.into())
                }
                input => {
                    let multiline = matches!(
                        input,
                        NativeInput::Text {
                            multiline: true,
                            ..
                        } | NativeInput::Json { .. }
                    );
                    let text = match input {
                        NativeInput::Text { .. } => value.as_str().ok_or_else(invalid)?.into(),
                        NativeInput::Json { .. } => {
                            serde_json::to_string_pretty(&value).map_err(|_| invalid())?
                        }
                        _ => value.to_string(),
                    };
                    if multiline {
                        let editor =
                            cx.new(|cx| TextareaState::new(window, cx).rows(4).default_value(text));
                        subscriptions.push(cx.observe(&editor, |_, _, cx| cx.notify()));
                        Draft::Multiline(editor)
                    } else {
                        let editor = cx.new(|cx| InputState::new(window, cx).default_value(text));
                        subscriptions.push(cx.observe(&editor, |_, _, cx| cx.notify()));
                        Draft::Input(editor)
                    }
                }
            };
            fields.push(Field {
                model: field.clone(),
                draft,
            });
        }
        self.fields = fields;
        self.form_generation = self.form_generation.wrapping_add(1);
        self.subscriptions = subscriptions;
        self.model = Some(model);
        Ok(())
    }

    pub(super) fn values(&self, cx: &App) -> Result<Value, PluginFailure> {
        let invalid = || PluginFailure::new("plugin_view_input_invalid");
        let mut values = Map::new();
        let mut text_bytes = 0;
        for field in &self.fields {
            let value = match &field.draft {
                Draft::Boolean(value) => Value::Bool(*value),
                Draft::Choice(value) => Value::String(value.clone()),
                Draft::Input(editor) => {
                    let text = editor.read(cx).value();
                    text_bytes += text.len();
                    if text.len() > 64 * 1024
                        || text_bytes as u64 > yss_plugin_runtime::MAX_NATIVE_VIEW_BYTES
                    {
                        return Err(invalid());
                    }
                    if matches!(field.model.input, NativeInput::Text { .. }) {
                        Value::String(text.to_string())
                    } else {
                        serde_json::from_str(text.as_ref()).map_err(|_| invalid())?
                    }
                }
                Draft::Multiline(editor) => {
                    let text = editor.read(cx).value();
                    text_bytes += text.len();
                    if text.len() > 64 * 1024
                        || text_bytes as u64 > yss_plugin_runtime::MAX_NATIVE_VIEW_BYTES
                    {
                        return Err(invalid());
                    }
                    if matches!(field.model.input, NativeInput::Text { .. }) {
                        Value::String(text.to_string())
                    } else {
                        serde_json::from_str(text.as_ref()).map_err(|_| invalid())?
                    }
                }
            };
            if !field.model.input.accepts(&value) {
                return Err(invalid());
            }
            values.insert(field.model.id.clone(), value);
        }
        Ok(Value::Object(values))
    }
}
