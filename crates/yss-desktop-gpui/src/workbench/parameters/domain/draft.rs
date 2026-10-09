//! One ordered draft owns codes, labels and the selected positive row.
use crate::{
    text::translate,
    workbench::parameters::{ParameterForm, field::subscribe_input},
};
use gpui::{App, AppContext, Context, Entity, EntityId, Subscription, Window};
use gpui_component::input::InputState;
use serde_json::Value;
use std::{borrow::Cow, cell::OnceCell, collections::HashSet};
use yss_data_contract::{ConversionDomain, SemanticValue};
use yss_graph_editor::projection::EditorParameterModel;
use yss_node_protocol::ParameterKey;

const PAGE_SIZE: usize = 50;

enum Row {
    Value(SemanticValue),
    Input(Inputs),
}

pub(super) struct Inputs {
    pub(super) code: Entity<InputState>,
    pub(super) label: Entity<InputState>,
    _subscriptions: [Subscription; 2],
}

pub(in crate::workbench::parameters) struct DomainDraft {
    rows: Vec<Row>,
    key: ParameterKey,
    pub(super) positive: Option<usize>,
    pub(super) page: usize,
    duplicates: OnceCell<bool>,
}

impl DomainDraft {
    pub(in crate::workbench::parameters) fn new(
        model: &EditorParameterModel,
        window: &mut Window,
        cx: &mut Context<ParameterForm>,
    ) -> Self {
        let domain = model
            .value
            .as_ref()
            .and_then(|value| serde_json::from_value::<ConversionDomain>(value.clone()).ok())
            .unwrap_or_default();
        let positive = domain.positive_value.as_ref().and_then(|positive| {
            domain
                .values
                .iter()
                .position(|value| &value.value == positive)
        });
        let mut draft = Self {
            rows: domain.values.into_iter().map(Row::Value).collect(),
            key: model.key.clone(),
            positive,
            page: 0,
            duplicates: OnceCell::new(),
        };
        draft.show_page(0, window, cx);
        draft
    }

    pub(super) fn len(&self) -> usize {
        self.rows.len()
    }
    pub(super) fn pages(&self) -> usize {
        self.len().div_ceil(PAGE_SIZE).max(1)
    }
    pub(super) fn visible(&self) -> impl Iterator<Item = (usize, &Inputs)> {
        self.rows
            .iter()
            .enumerate()
            .skip(self.page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .filter_map(|(index, row)| match row {
                Row::Input(inputs) => Some((index, inputs)),
                Row::Value(_) => None,
            })
    }

    pub(super) fn show_page(
        &mut self,
        page: usize,
        window: &mut Window,
        cx: &mut Context<ParameterForm>,
    ) {
        self.page = page.min(self.pages() - 1);
        for row in self
            .rows
            .iter_mut()
            .skip(self.page * PAGE_SIZE)
            .take(PAGE_SIZE)
        {
            if let Row::Value(value) = row {
                let code = cx.new(|cx| {
                    InputState::new(window, cx).default_value(std::mem::take(&mut value.value))
                });
                let label = cx.new(|cx| {
                    InputState::new(window, cx).default_value(std::mem::take(&mut value.label))
                });
                let subscriptions = [
                    subscribe_input(&code, self.key.clone(), window, cx),
                    subscribe_input(&label, self.key.clone(), window, cx),
                ];
                *row = Row::Input(Inputs {
                    code,
                    label,
                    _subscriptions: subscriptions,
                });
            }
        }
    }

    pub(super) fn add(&mut self, window: &mut Window, cx: &mut Context<ParameterForm>) {
        if self.len() >= ConversionDomain::MAX_VALUES {
            return;
        }
        self.rows.push(Row::Value(SemanticValue {
            value: String::new(),
            label: String::new(),
        }));
        self.duplicates.take();
        self.show_page(self.pages() - 1, window, cx);
    }

    pub(super) fn remove(
        &mut self,
        row: usize,
        window: &mut Window,
        cx: &mut Context<ParameterForm>,
    ) {
        if row >= self.len() {
            return;
        }
        self.rows.remove(row);
        self.positive = self.positive.and_then(|index| {
            if index == row {
                None
            } else {
                Some(if index > row { index - 1 } else { index })
            }
        });
        self.duplicates.take();
        self.show_page(self.page, window, cx);
    }

    pub(super) fn move_row(
        &mut self,
        row: usize,
        direction: isize,
        window: &mut Window,
        cx: &mut Context<ParameterForm>,
    ) {
        if let Some(target) = row.checked_add_signed(direction)
            && row < self.len()
            && target < self.len()
        {
            self.rows.swap(row, target);
            self.positive = self.positive.map(|index| {
                if index == row {
                    target
                } else if index == target {
                    row
                } else {
                    index
                }
            });
            self.show_page(target / PAGE_SIZE, window, cx);
        }
    }

    pub(super) fn code<'a>(&'a self, row: usize, cx: &App) -> Option<Cow<'a, str>> {
        self.rows.get(row).map(|row| match row {
            Row::Value(value) => Cow::Borrowed(value.value.as_str()),
            Row::Input(input) => Cow::Owned(input.code.read(cx).value().to_string()),
        })
    }

    pub(super) fn choice_label(&self, row: usize, cx: &App) -> String {
        let label = match &self.rows[row] {
            Row::Value(value) => value.label.clone(),
            Row::Input(input) => input.label.read(cx).value().to_string(),
        };
        if !label.is_empty() {
            return label;
        }
        let code = self.code(row, cx).unwrap_or_default();
        if code.is_empty() {
            "∅".into()
        } else {
            code.into_owned()
        }
    }

    pub(super) fn has_duplicates(&self, cx: &App) -> bool {
        *self.duplicates.get_or_init(|| {
            let mut codes = HashSet::with_capacity(self.len());
            (0..self.len()).any(|row| !codes.insert(self.code(row, cx).expect("existing row")))
        })
    }

    pub(in crate::workbench::parameters) fn input_changed(&mut self, id: EntityId) {
        if self
            .rows
            .iter()
            .any(|row| matches!(row, Row::Input(input) if input.code.entity_id() == id))
        {
            self.duplicates.take();
        }
    }

    pub(in crate::workbench::parameters) fn owns_input(&self, id: EntityId) -> bool {
        self.rows.iter().any(|row| matches!(row, Row::Input(input) if input.code.entity_id() == id || input.label.entity_id() == id))
    }

    pub(in crate::workbench::parameters) fn value(&self, cx: &App) -> Result<Value, String> {
        if self.has_duplicates(cx) {
            return Err(translate("conversion.duplicateValues"));
        }
        let values: Vec<_> = self
            .rows
            .iter()
            .map(|row| match row {
                Row::Value(value) => value.clone(),
                Row::Input(input) => SemanticValue {
                    value: input.code.read(cx).value().to_string(),
                    label: input.label.read(cx).value().to_string(),
                },
            })
            .collect();
        let positive_value = self
            .positive
            .and_then(|row| values.get(row))
            .map(|value| value.value.clone());
        let domain = ConversionDomain {
            values,
            positive_value,
        };
        if !domain.is_valid() {
            return Err(translate("native.workbench.invalidDomainMapping"));
        }
        serde_json::to_value(domain).map_err(|_| translate("native.workbench.domainMappingFailed"))
    }
}
