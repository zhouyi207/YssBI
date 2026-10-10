//! Only the visible mapping page owns input entities; the dialog owns the unsaved semantic draft.
use super::SemanticDialog;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{App, AppContext, Context, Entity, Subscription, Window};
use std::ops::Range;
use yss_data_contract::{ConversionDomain, NumericConstraints, SemanticType, SemanticValue};

pub(super) const PAGE_VALUES: usize = 50;

pub(super) struct MappingInputs {
    pub value: Entity<InputState>,
    pub label: Entity<InputState>,
    _subscriptions: [Subscription; 2],
}

impl SemanticDialog {
    pub(super) fn editable(&self) -> bool {
        !self.loading
            && !self.saving
            && self.values_ready
            && self.error != Some("detail.data.settingsStale")
    }

    pub(super) fn page_range(&self) -> Range<usize> {
        let start = self.page * PAGE_VALUES;
        start..(start + PAGE_VALUES).min(self.draft.values.len())
    }

    pub(super) fn mount_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for index in self.page_range() {
            self.inputs.entry(index).or_insert_with(|| {
                let entry = &self.draft.values[index];
                let value =
                    cx.new(|cx| InputState::new(window, cx).default_value(entry.value.clone()));
                let label =
                    cx.new(|cx| InputState::new(window, cx).default_value(entry.label.clone()));
                let subscriptions =
                    [(true, &value), (false, &label)].map(|(stored_value, input)| {
                        cx.subscribe(input, move |view, source, event, cx| {
                            if !matches!(event, InputEvent::Change) {
                                return;
                            }
                            // Ignore input events from a page whose controls have been released.
                            let Some(current) = view.inputs.get(&index) else {
                                return;
                            };
                            let current = if stored_value {
                                &current.value
                            } else {
                                &current.label
                            };
                            if source.entity_id() != current.entity_id() {
                                return;
                            }
                            let text = source.read(cx).value().to_string();
                            let entry = &mut view.draft.values[index];
                            if stored_value {
                                if view.draft.positive_value.as_ref() == Some(&entry.value) {
                                    view.draft.positive_value = Some(text.clone());
                                }
                                entry.value = text;
                            } else {
                                entry.label = text;
                            }
                            view.clear_feedback();
                            cx.notify();
                        })
                    });
                MappingInputs {
                    value,
                    label,
                    _subscriptions: subscriptions,
                }
            });
        }
    }

    pub(super) fn flush_inputs(&mut self, cx: &App) {
        // Resolve the positive entry before editing any values, including temporary duplicates.
        let positive = self.draft.positive_value.as_ref().and_then(|value| {
            self.draft
                .values
                .iter()
                .position(|entry| &entry.value == value)
        });
        for (index, input) in &self.inputs {
            let entry = &mut self.draft.values[*index];
            entry.value = input.value.read(cx).value().to_string();
            entry.label = input.label.read(cx).value().to_string();
            if Some(*index) == positive {
                self.draft.positive_value = Some(entry.value.clone());
            }
        }
        if self.draft.kind == SemanticType::Numeric {
            let minimum = self.minimum.read(cx).value().to_string();
            let maximum = self.maximum.read(cx).value().to_string();
            let integer = self
                .draft
                .numeric
                .as_ref()
                .is_some_and(|value| value.integer);
            if self.draft.numeric.is_some() || integer || !minimum.is_empty() || !maximum.is_empty()
            {
                self.draft.numeric = Some(NumericConstraints {
                    integer,
                    minimum: (!minimum.trim().is_empty()).then_some(minimum),
                    maximum: (!maximum.trim().is_empty()).then_some(maximum),
                });
            }
        }
    }

    pub(super) fn clear_feedback(&mut self) {
        if self.error != Some("detail.data.settingsStale") && self.values_ready {
            self.error = None;
        }
    }

    pub(super) fn show_page(&mut self, page: usize, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        self.flush_inputs(cx);
        self.inputs.clear();
        self.page = page.min(self.draft.values.len().saturating_sub(1) / PAGE_VALUES);
        cx.notify();
    }

    pub(super) fn reorder(&mut self, index: usize, delta: isize, cx: &mut Context<Self>) {
        let Some(target) = index.checked_add_signed(delta) else {
            return;
        };
        if !self.editable() || index >= self.draft.values.len() || target >= self.draft.values.len()
        {
            return;
        }
        self.flush_inputs(cx);
        self.draft.values.swap(index, target);
        self.inputs.clear();
        self.page = target / PAGE_VALUES;
        self.clear_feedback();
        cx.notify();
    }

    pub(super) fn add_value(&mut self, cx: &mut Context<Self>) {
        if !self.editable()
            || self.draft.values.len() >= ConversionDomain::MAX_VALUES
            || (self.draft.kind == SemanticType::Binary && self.draft.values.len() >= 2)
        {
            return;
        }
        self.flush_inputs(cx);
        self.page = self.draft.values.len() / PAGE_VALUES;
        self.draft.values.push(SemanticValue {
            value: String::new(),
            label: String::new(),
        });
        self.inputs.clear();
        self.clear_feedback();
        cx.notify();
    }

    pub(super) fn remove_value(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.editable() || index >= self.draft.values.len() {
            return;
        }
        self.flush_inputs(cx);
        let removed = self.draft.values.remove(index);
        if self.draft.positive_value.as_ref() == Some(&removed.value) {
            self.draft.positive_value = None;
        }
        self.inputs.clear();
        self.page = self
            .page
            .min(self.draft.values.len().saturating_sub(1) / PAGE_VALUES);
        self.clear_feedback();
        cx.notify();
    }
}
