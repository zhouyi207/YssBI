//! Provider and model input buffers; validation and persistence stay with the existing service.
mod provider;
mod render;

use super::{Page, SettingsPanel};
use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::{App, Context, Entity, Window};
use yss_harness_contract::{
    LanguageModelAuthentication, LanguageModelConfig, LanguageModelProtocol,
    LanguageModelProviderConfig, LanguageModelProviderPreset, LanguageModelProviderStatus,
    ReasoningEffort,
};

pub(super) struct ProviderDraft {
    pub id: String,
    pub name: String,
    pub preset: Entity<provider::ProviderPicker>,
    pub custom_name: Entity<InputState>,
    pub base_url: Entity<InputState>,
    pub key: Entity<InputState>,
    pub protocol: LanguageModelProtocol,
    pub adapter: String,
    pub authentication: LanguageModelAuthentication,
    pub models: Vec<LanguageModelConfig>,
    pub changed: bool,
    pub saved: bool,
}
pub(super) struct ModelDraft {
    pub index: Option<usize>,
    pub id: Entity<InputState>,
    pub name: Entity<InputState>,
    pub context: Entity<InputState>,
    pub output: Entity<InputState>,
    pub temperature: Entity<InputState>,
    pub top_p: Entity<InputState>,
    pub parameters: Entity<TextareaState>,
    pub efforts: Vec<ReasoningEffort>,
    pub changed: bool,
}

impl ProviderDraft {
    pub(super) fn display_name(&self, cx: &App) -> String {
        let custom = self.custom_name.read(cx).value();
        if custom.trim().is_empty() {
            self.name.clone()
        } else {
            custom.trim().to_owned()
        }
    }

    pub(super) fn configuration(&self, cx: &App) -> LanguageModelProviderConfig {
        let custom = self.custom_name.read(cx).value().trim().to_owned();
        LanguageModelProviderConfig {
            id: self.id.clone(),
            name: self.name.clone(),
            custom_name: (!custom.is_empty()).then_some(custom),
            protocol: self.protocol,
            adapter: self.adapter.clone(),
            authentication: self.authentication,
            base_url: self.base_url.read(cx).value().trim().to_owned(),
            models: self.models.clone(),
        }
    }
}
impl ModelDraft {
    fn configuration(&self, cx: &App) -> Result<LanguageModelConfig, String> {
        let parameters = self.parameters.read(cx).value();
        let additional_parameters = if parameters.trim().is_empty() {
            Default::default()
        } else {
            serde_json::from_str(parameters.as_ref()).map_err(|_| {
                crate::text::t("native.settings.parametersObjectRequired").into_owned()
            })?
        };
        let id = self.id.read(cx).value().trim().to_owned();
        let name = self.name.read(cx).value().trim().to_owned();
        Ok(LanguageModelConfig {
            name: if name.is_empty() { id.clone() } else { name },
            id,
            context_window: optional(
                &self.context,
                &crate::text::t("native.settings.contextCapacity"),
                cx,
            )?,
            max_output_tokens: optional(
                &self.output,
                &crate::text::t("native.settings.maxOutput"),
                cx,
            )?,
            temperature: optional(&self.temperature, "Temperature", cx)?,
            top_p: optional(&self.top_p, "Top P", cx)?,
            additional_parameters,
            reasoning_efforts: self.efforts.clone(),
        })
    }
}

fn optional<T: std::str::FromStr>(
    input: &Entity<InputState>,
    name: &str,
    cx: &App,
) -> Result<Option<T>, String> {
    let value = input.read(cx).value();
    if value.trim().is_empty() {
        Ok(None)
    } else {
        value.trim().parse().map(Some).map_err(|_| {
            crate::text::format(
                "native.settings.invalidField",
                &[("name", name.to_string())],
            )
        })
    }
}

impl SettingsPanel {
    pub(super) fn edit_provider(
        &mut self,
        existing: Option<LanguageModelProviderStatus>,
        preset: Option<LanguageModelProviderPreset>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() || self.dirty() {
            self.error = Some(crate::text::t("native.settings.saveOrDiscard").into());
            cx.notify();
            return;
        }
        self.discard(cx);
        let saved = existing.is_some();
        let has_api_key = existing.as_ref().is_some_and(|entry| entry.has_api_key);
        let config = existing.map(|entry| entry.config).unwrap_or_else(|| {
            let preset = preset.expect("new provider comes from an original catalog preset");
            LanguageModelProviderConfig {
                id: uuid::Uuid::new_v4().to_string(),
                name: preset.name,
                custom_name: None,
                protocol: preset.protocol,
                adapter: preset.adapter,
                authentication: preset.authentication,
                base_url: preset.base_url,
                models: vec![],
            }
        });
        let picker = self.provider_picker(&config, window, cx);
        self.editor = Some(ProviderDraft {
            id: config.id,
            name: config.name,
            preset: picker,
            custom_name: self.field(config.custom_name.unwrap_or_default(), None, window, cx),
            base_url: self.field(config.base_url, None, window, cx),
            key: self.field(
                String::new(),
                Some(&if has_api_key {
                    "********".into()
                } else {
                    crate::text::t("settings.models.enterKey")
                }),
                window,
                cx,
            ),
            protocol: config.protocol,
            adapter: config.adapter,
            authentication: config.authentication,
            models: config.models,
            changed: !saved,
            saved,
        });
        self.page = Page::Provider;
        cx.notify();
    }

    fn field(
        &mut self,
        value: String,
        masked: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        use gpui_kit::AppContext;
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(value)
                .masked(masked.is_some())
                .placeholder(masked.unwrap_or_default())
        });
        let epoch = self.epoch;
        self.provider_subscriptions
            .push(cx.subscribe(&input, move |view, _, event, cx| {
                if matches!(event, InputEvent::Change) && view.epoch == epoch {
                    if let Some(draft) = &mut view.editor {
                        draft.changed = true;
                    }
                    view.error = None;
                    view.feedback = None;
                    cx.notify();
                }
            }));
        input
    }

    fn edit_model(&mut self, index: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_kit::AppContext;
        if self.busy() {
            return;
        }
        if self.model.as_ref().is_some_and(|draft| draft.changed) {
            self.error = Some(crate::text::t("native.settings.applyOrCancelModel").into());
            cx.notify();
            return;
        }
        let Some(provider) = &self.editor else {
            return;
        };
        if index.is_some_and(|index| provider.models.get(index).is_none()) {
            return;
        }
        let config = index
            .and_then(|index| provider.models.get(index))
            .cloned()
            .unwrap_or(LanguageModelConfig {
                id: String::new(),
                name: String::new(),
                context_window: None,
                max_output_tokens: None,
                temperature: None,
                top_p: None,
                additional_parameters: Default::default(),
                reasoning_efforts: vec![],
            });
        self.model_subscriptions.clear();
        let parameters =
            serde_json::to_string_pretty(&config.additional_parameters).unwrap_or_default();
        let parameters = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 12)
                .default_value(parameters)
        });
        let epoch = self.epoch;
        self.model_subscriptions
            .push(cx.subscribe(&parameters, move |view, _, event, cx| {
                if matches!(event, InputEvent::Change) && view.epoch == epoch {
                    if let Some(draft) = &mut view.model {
                        draft.changed = true;
                    }
                    view.error = None;
                    cx.notify();
                }
            }));
        self.model = Some(ModelDraft {
            index,
            id: self.model_field(config.id, window, cx),
            name: self.model_field(config.name, window, cx),
            context: self.model_field(
                config
                    .context_window
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
                window,
                cx,
            ),
            output: self.model_field(
                config
                    .max_output_tokens
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
                window,
                cx,
            ),
            temperature: self.model_field(
                config
                    .temperature
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
                window,
                cx,
            ),
            top_p: self.model_field(
                config.top_p.map(|n| n.to_string()).unwrap_or_default(),
                window,
                cx,
            ),
            parameters,
            efforts: config.reasoning_efforts,
            changed: index.is_none(),
        });
        self.error = None;
        cx.notify();
    }

    fn model_field(
        &mut self,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        use gpui_kit::AppContext;
        let input = cx.new(|cx| InputState::new(window, cx).default_value(value));
        let epoch = self.epoch;
        self.model_subscriptions
            .push(cx.subscribe(&input, move |view, _, event, cx| {
                if matches!(event, InputEvent::Change) && view.epoch == epoch {
                    if let Some(draft) = &mut view.model {
                        draft.changed = true;
                    }
                    view.error = None;
                    cx.notify();
                }
            }));
        input
    }

    pub(super) fn configuration(&self, cx: &App) -> Result<LanguageModelProviderConfig, String> {
        let provider = self
            .editor
            .as_ref()
            .ok_or_else(|| crate::text::t("native.settings.chooseProvider").into_owned())?;
        let mut config = provider.configuration(cx);
        if let Some(model) = &self.model {
            let value = model.configuration(cx)?;
            if let Some(index) = model.index {
                config.models[index] = value;
            } else {
                config.models.push(value);
            }
        }
        if !config.validate() {
            return Err(crate::text::t("native.settings.invalidProvider").into());
        }
        Ok(config)
    }

    fn apply_model(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(model) = &self.model else {
            return;
        };
        let index = model.index;
        let outcome = model.configuration(cx).and_then(|config| {
            let provider = self.editor.as_ref().unwrap();
            if !config.validate(provider.protocol)
                || provider
                    .models
                    .iter()
                    .enumerate()
                    .any(|(other, existing)| Some(other) != index && existing.id == config.id)
            {
                return Err(crate::text::t("native.settings.invalidModel").into());
            }
            Ok(config)
        });
        match outcome {
            Ok(config) => {
                let provider = self.editor.as_mut().unwrap();
                if let Some(index) = index {
                    provider.models[index] = config;
                } else {
                    provider.models.push(config);
                }
                provider.changed = true;
                self.model = None;
                self.model_subscriptions.clear();
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
        cx.notify();
    }

    pub(super) fn navigate(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        if !self.dirty() {
            self.discard(cx);
            self.page = page;
            self.load_knowledge(cx);
            return;
        }
        let epoch = self.epoch;
        let owner = cx.entity().downgrade();
        crate::modal_window::confirm(
            crate::text::t("native.settings.discardTitle"),
            crate::text::t("native.settings.discardMessage"),
            crate::text::t("editor.close.discard"),
            crate::text::t("native.imports.keepEditing"),
            window,
            cx,
            move |_, _, cx| {
                owner
                    .update(cx, |view, cx| {
                        if view.epoch == epoch && !view.busy() {
                            view.discard(cx);
                            view.page = page;
                            view.load_knowledge(cx);
                            cx.notify();
                        }
                    })
                    .is_ok()
            },
        );
    }

    fn model_row_current(&self, epoch: u64, index: usize, id: &str) -> bool {
        self.epoch == epoch
            && !self.busy()
            && self
                .editor
                .as_ref()
                .and_then(|draft| draft.models.get(index))
                .is_some_and(|model| model.id == id)
    }
}

pub(super) fn provider_name(config: &LanguageModelProviderConfig) -> String {
    config
        .custom_name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(&config.name)
        .trim()
        .to_owned()
}
