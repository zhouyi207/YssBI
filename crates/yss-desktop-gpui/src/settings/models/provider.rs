//! Preset selection and connection edits over the Application model catalog.
mod render;

use crate::settings::SettingsPanel;
use gpui_kit::component::{
    IndexPath,
    combobox::{ComboboxEvent, ComboboxState},
    searchable_list::{SearchableListItem, SearchableVec},
};
use gpui_kit::{AppContext, Context, Entity, SharedString, Window};
use yss_harness_contract::{
    LanguageModelAuthentication, LanguageModelProtocol, LanguageModelProviderConfig,
    LanguageModelProviderStatus,
};

#[derive(Clone)]
pub(in crate::settings) struct ProviderChoice {
    id: String,
    label: SharedString,
}

impl SearchableListItem for ProviderChoice {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &String {
        &self.id
    }
}

pub(in crate::settings) type ProviderPicker = ComboboxState<SearchableVec<ProviderChoice>>;

impl SettingsPanel {
    pub(super) fn provider_picker(
        &mut self,
        config: &LanguageModelProviderConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<ProviderPicker> {
        let presets = self
            .catalog
            .as_ref()
            .map(|catalog| catalog.presets.as_slice())
            .unwrap_or_default();
        let selected = presets
            .iter()
            .position(|preset| preset.name == config.name)
            .map(IndexPath::new)
            .into_iter()
            .collect();
        let items: Vec<_> = presets
            .iter()
            .map(|preset| ProviderChoice {
                id: preset.id.clone(),
                label: preset.name.clone().into(),
            })
            .collect();
        let picker = cx.new(|cx| {
            ComboboxState::new(SearchableVec::new(items), selected, window, cx).searchable(true)
        });
        let epoch = self.epoch;
        self.provider_subscriptions.push(cx.subscribe_in(
            &picker,
            window,
            move |view, _, event: &ComboboxEvent<SearchableVec<ProviderChoice>>, window, cx| {
                if view.epoch == epoch
                    && let ComboboxEvent::Change(values) = event
                    && let Some(id) = values.first()
                {
                    view.select_provider(id, window, cx);
                }
            },
        ));
        picker
    }

    fn select_provider(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let preset = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.presets.iter().find(|preset| preset.id == id))
            .cloned();
        if self.busy() || preset.is_none() {
            // A queued selection may arrive after a save starts; restore the displayed draft.
            if let Some(draft) = &self.editor {
                let values = self
                    .catalog
                    .as_ref()
                    .and_then(|catalog| {
                        catalog
                            .presets
                            .iter()
                            .find(|preset| preset.name == draft.name)
                    })
                    .map(|preset| preset.id.clone())
                    .into_iter()
                    .collect::<Vec<_>>();
                draft.preset.update(cx, |picker, cx| {
                    picker.set_selected_values(&values, window, cx)
                });
            }
            return;
        }
        let Some(draft) = &mut self.editor else {
            return;
        };
        let preset = preset.unwrap();
        draft.name = preset.name;
        draft.protocol = preset.protocol;
        draft.adapter = preset.adapter;
        draft.authentication = preset.authentication;
        draft
            .base_url
            .update(cx, |input, cx| input.set_value(preset.base_url, window, cx));
        draft
            .key
            .update(cx, |input, cx| input.set_value("", window, cx));
        draft.models.clear();
        draft.changed = true;
        self.model = None;
        self.model_subscriptions.clear();
        self.error = None;
        self.feedback = None;
        self.refresh_key_placeholder(window, cx);
        cx.notify();
    }

    fn change_protocol(
        &mut self,
        protocol: LanguageModelProtocol,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() {
            return;
        }
        let Some(draft) = &mut self.editor else {
            return;
        };
        if draft.protocol == protocol {
            return;
        }
        let (preset_id, family) = match protocol {
            LanguageModelProtocol::OpenAiChat | LanguageModelProtocol::OpenAiResponses => {
                ("openai", "/openai")
            }
            LanguageModelProtocol::Anthropic => ("anthropic", "/anthropic"),
            LanguageModelProtocol::Gemini => ("gemini", "/gemini"),
        };
        if !draft.adapter.ends_with(family) {
            // Adapter IDs and defaults come from the original preset owner, not a second catalog.
            let Some(preset) = self
                .catalog
                .as_ref()
                .and_then(|catalog| catalog.presets.iter().find(|preset| preset.id == preset_id))
            else {
                return;
            };
            draft.adapter.clone_from(&preset.adapter);
        }
        draft.protocol = protocol;
        if !matches!(
            protocol,
            LanguageModelProtocol::OpenAiChat | LanguageModelProtocol::OpenAiResponses
        ) {
            draft.authentication = LanguageModelAuthentication::ApiKey;
        }
        draft.changed = true;
        self.error = None;
        self.feedback = None;
        self.refresh_key_placeholder(window, cx);
        cx.notify();
    }

    fn change_authentication(
        &mut self,
        authentication: LanguageModelAuthentication,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() {
            return;
        }
        let Some(draft) = &mut self.editor else {
            return;
        };
        if draft.authentication == authentication
            || (authentication == LanguageModelAuthentication::None
                && !matches!(
                    draft.protocol,
                    LanguageModelProtocol::OpenAiChat | LanguageModelProtocol::OpenAiResponses
                ))
        {
            return;
        }
        draft.authentication = authentication;
        draft
            .key
            .update(cx, |input, cx| input.set_value("", window, cx));
        draft.changed = true;
        self.error = None;
        self.feedback = None;
        self.refresh_key_placeholder(window, cx);
        cx.notify();
    }

    fn stored_provider(&self) -> Option<&LanguageModelProviderStatus> {
        let draft = self.editor.as_ref()?;
        self.catalog
            .as_ref()?
            .providers
            .iter()
            .find(|provider| provider.config.id == draft.id)
    }

    pub(in crate::settings) fn stored_key_available(&self) -> bool {
        self.editor.as_ref().is_some_and(|draft| {
            self.stored_provider().is_some_and(|stored| {
                stored.has_api_key
                    && stored.config.matches_credential_scope(
                        &draft.id,
                        &draft.name,
                        &draft.adapter,
                    )
            })
        })
    }

    pub(in crate::settings) fn replacement_key_required(&self) -> bool {
        self.editor
            .as_ref()
            .is_some_and(|draft| draft.authentication == LanguageModelAuthentication::ApiKey)
            && self
                .stored_provider()
                .is_some_and(|stored| stored.has_api_key)
            && !self.stored_key_available()
    }

    pub(in crate::settings) fn connection_ready(&self, cx: &gpui_kit::App) -> bool {
        self.editor.as_ref().is_some_and(|draft| {
            !draft.name.trim().is_empty()
                && !draft.base_url.read(cx).value().trim().is_empty()
                && (draft.authentication == LanguageModelAuthentication::None
                    || !draft.key.read(cx).value().trim().is_empty()
                    || self.stored_key_available())
        })
    }

    pub(in crate::settings) fn refresh_key_placeholder(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(draft) = &self.editor {
            let placeholder = if self.stored_key_available() {
                "********".to_owned()
            } else {
                crate::text::translate("settings.models.enterKey")
            };
            if draft.key.read(cx).presentation().placeholder().as_ref() != placeholder.as_str() {
                draft.key.update(cx, |input, cx| {
                    input.set_placeholder(placeholder, window, cx)
                });
            }
        }
    }
}

fn protocol_label(protocol: LanguageModelProtocol) -> &'static str {
    match protocol {
        LanguageModelProtocol::OpenAiResponses => "OpenAI Responses",
        LanguageModelProtocol::OpenAiChat => "OpenAI Chat Completions",
        LanguageModelProtocol::Anthropic => "Anthropic Messages",
        LanguageModelProtocol::Gemini => "Gemini Interactions",
    }
}
