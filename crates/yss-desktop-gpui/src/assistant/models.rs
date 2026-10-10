//! Searchable model choices project the catalog; session receipts own the selection.
use super::{ConversationEvent, ConversationPanel};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Selectable, Sizable,
    button::{Button, ButtonVariants},
    combobox::{Combobox, ComboboxEvent, ComboboxState},
    searchable_list::{SearchableListItem, SearchableVec},
};
use gpui_kit::{
    AnyElement, App, Context, Entity, Focusable, SharedString, Subscription, WeakEntity, Window,
    div, prelude::*, px,
};
use std::sync::Arc;
use yss_harness_contract::{
    LanguageModelAuthentication, LanguageModelCatalog, LanguageModelConfig,
    LanguageModelProviderStatus, LanguageModelSelection,
};

#[derive(Clone)]
struct Choice {
    value: LanguageModelSelection,
    label: SharedString,
    enabled: bool,
    selected: bool,
}

impl SearchableListItem for Choice {
    type Value = LanguageModelSelection;

    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &Self::Value {
        &self.value
    }
    fn disabled(&self) -> bool {
        !self.enabled
    }
    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .min_w_0()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(self.label.clone()),
            )
            .when(self.selected, |row| {
                row.child(Icon::new(IconName::Check).xsmall())
            })
            .when(!self.enabled, |row| {
                row.child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::text::t("settings.models.needsKey")),
                )
            })
    }
}

struct Picker {
    catalog: Option<Arc<LanguageModelCatalog>>,
    selected: Option<LanguageModelSelection>,
    control: Entity<ComboboxState<SearchableVec<Choice>>>,
    _selection: Subscription,
}

impl Picker {
    fn new(
        owner: WeakEntity<ConversationPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let control: Entity<ComboboxState<SearchableVec<Choice>>> = cx.new(|cx| {
            ComboboxState::new(SearchableVec::new(vec![]), vec![], window, cx).searchable(true)
        });
        let subscription = cx.subscribe_in(&control, window, move |_, _, event, window, cx| {
            if let ComboboxEvent::Change(values) = event
                && let Some(value) = values.first()
            {
                let _ = owner.update(cx, |view, cx| view.select_model(value.clone(), window, cx));
            }
        });
        Self {
            catalog: None,
            selected: None,
            control,
            _selection: subscription,
        }
    }

    fn sync(
        &mut self,
        catalog: Option<&Arc<LanguageModelCatalog>>,
        selected: Option<LanguageModelSelection>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = self.selected != selected
            || match (&self.catalog, catalog) {
                (Some(previous), Some(current)) => !Arc::ptr_eq(previous, current),
                (None, None) => false,
                _ => true,
            };
        if !changed && self.control.read(cx).selected_value().is_none() {
            return;
        }
        let query = self.control.read(cx).query(cx);
        self.control.update(cx, |control, cx| {
            if changed {
                let choices = catalog
                    .into_iter()
                    .flat_map(|catalog| &catalog.providers)
                    .flat_map(|provider| {
                        let selected = selected.as_ref();
                        provider.config.models.iter().map(move |model| Choice {
                            value: LanguageModelSelection {
                                provider_id: provider.config.id.clone(),
                                model_id: model.id.clone(),
                            },
                            label: format!("{} · {}", provider_name(provider), model.name).into(),
                            enabled: provider_available(provider),
                            selected: selected.is_some_and(|selected| {
                                selected.provider_id == provider.config.id
                                    && selected.model_id == model.id
                            }),
                        })
                    })
                    .collect::<Vec<_>>();
                control.set_items(SearchableVec::new(choices), window, cx);
            }
            // This is a command picker: the session supplies the checkmark. Keeping
            // its transient selection empty also avoids index-based equality across
            // filtered lists in Combobox; typed identities are validated by the panel.
            control.set_selected_values(&[], window, cx);
            control.set_query(query, window, cx);
        });
        self.catalog = catalog.cloned();
        self.selected = selected;
    }
}

impl ConversationPanel {
    pub(super) fn model_config(
        &self,
        selection: &LanguageModelSelection,
    ) -> Option<(&LanguageModelProviderStatus, &LanguageModelConfig)> {
        let provider = self
            .catalog
            .as_ref()?
            .providers
            .iter()
            .find(|provider| provider.config.id == selection.provider_id)?;
        let model = provider
            .config
            .models
            .iter()
            .find(|model| model.id == selection.model_id)?;
        Some((provider, model))
    }

    pub(super) fn model_available_for(&self, selection: &LanguageModelSelection) -> bool {
        self.model_config(selection)
            .is_some_and(|(provider, _)| provider_available(provider))
    }

    pub(super) fn model_available(&self) -> bool {
        self.selection()
            .is_some_and(|selection| self.model_available_for(&selection))
    }

    pub(super) fn model_picker(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let selection = self.selection();
        let current = selection
            .as_ref()
            .and_then(|selection| self.model_config(selection));
        let (label, hint) = current.map_or_else(
            || {
                let label = crate::text::t(if selection.is_some() {
                    "settings.models.unavailable"
                } else {
                    "settings.models.chooseModel"
                })
                .into_owned();
                (label.clone(), label)
            },
            |(provider, model)| {
                (
                    model.name.clone(),
                    format!("{} · {}", provider_name(provider), model.name),
                )
            },
        );
        let owner = cx.entity().downgrade();
        // Window state releases the query and popup when this conversation is hidden.
        let picker = window.use_keyed_state("assistant-model-picker", cx, |window, cx| {
            Picker::new(owner.clone(), window, cx)
        });
        let control = picker.update(cx, |picker, cx| {
            picker.sync(
                self.catalog.as_ref(),
                selection.filter(|_| current.is_some()),
                window,
                cx,
            );
            picker.control.clone()
        });
        let control = Combobox::new(&control)
            .xsmall()
            .appearance(false)
            .p_0()
            .w_full()
            .h_full()
            .menu_width(px(320.))
            .menu_max_h(px(300.))
            .disabled(!self.ready || self.selecting)
            .search_placeholder(crate::text::t("panel.assistantSearchModels"))
            .empty(|_, cx| {
                div()
                    .p_3()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::t("panel.assistantNoModels"))
            })
            .render_trigger(move |trigger, _, _| {
                Button::new("assistant-model")
                    .xsmall()
                    .ghost()
                    .max_w(px(144.))
                    .label(label.clone())
                    .tooltip(hint.clone())
                    .accessibility_label(crate::text::t("settings.models.chooseModel"))
                    .dropdown_caret(true)
                    .selected(trigger.is_open())
                    .disabled(trigger.is_disabled())
            })
            .footer(move |_, _| {
                let owner = owner.clone();
                Button::new("assistant-manage-models")
                    .xsmall()
                    .ghost()
                    .w_full()
                    .label(crate::text::t("settings.models.manage"))
                    .on_click(move |_, window, cx| {
                        let _ = owner.update(cx, |view, cx| {
                            view.input.focus_handle(cx).focus(window, cx);
                            cx.emit(ConversationEvent::Settings);
                        });
                    })
            })
            .into_any_element();
        div()
            .w(px(144.))
            .h(px(20.))
            .flex_shrink_0()
            .child(control)
            .into_any_element()
    }
}

fn provider_name(provider: &LanguageModelProviderStatus) -> &str {
    provider
        .config
        .custom_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(provider.config.name.trim())
}

fn provider_available(provider: &LanguageModelProviderStatus) -> bool {
    provider.has_api_key || provider.config.authentication == LanguageModelAuthentication::None
}
