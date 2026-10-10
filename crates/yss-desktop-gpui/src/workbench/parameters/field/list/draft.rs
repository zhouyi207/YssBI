//! A series owns one draft per row; input entities are created on visited pages.
use super::super::{ParameterForm, display, subscribe_input};
use gpui_kit::component::input::InputState;
use gpui_kit::{App, AppContext, Context, Entity, EntityId, Subscription, Window};
use serde_json::Value;
use yss_data_contract::{SemanticType, ValueType};
use yss_graph_editor::projection::EditorParameterModel;
use yss_node_protocol::ParameterKey;

pub(super) const PAGE_SIZE: usize = 50;

enum Row {
    Text(String),
    Input {
        input: Entity<InputState>,
        _subscription: Subscription,
    },
}

pub(in crate::workbench::parameters) struct ListDraft {
    rows: Vec<Row>,
    numeric: bool,
    key: ParameterKey,
    pub(super) page: usize,
}

impl ListDraft {
    pub(in crate::workbench::parameters::field) fn new(
        model: &EditorParameterModel,
        window: &mut Window,
        cx: &mut Context<ParameterForm>,
    ) -> Self {
        Self::from_strings(
            &model.key,
            model
                .value
                .as_ref()
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(display),
            model.value_type
                == Some(ValueType::DataSeries(Box::new(ValueType::Scalar(
                    SemanticType::Numeric,
                )))),
            window,
            cx,
        )
    }

    pub(in crate::workbench::parameters) fn text(
        key: &ParameterKey,
        values: impl IntoIterator<Item = String>,
        window: &mut Window,
        cx: &mut Context<ParameterForm>,
    ) -> Self {
        Self::from_strings(key, values, false, window, cx)
    }

    fn from_strings(
        key: &ParameterKey,
        values: impl IntoIterator<Item = String>,
        numeric: bool,
        window: &mut Window,
        cx: &mut Context<ParameterForm>,
    ) -> Self {
        let mut draft = Self {
            rows: values.into_iter().map(Row::Text).collect(),
            numeric,
            key: key.clone(),
            page: 0,
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

    pub(super) fn visible(&self) -> impl Iterator<Item = (usize, &Entity<InputState>)> {
        self.rows
            .iter()
            .enumerate()
            .skip(self.page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .filter_map(|(row, entry)| match entry {
                Row::Input { input, .. } => Some((row, input)),
                Row::Text(_) => None,
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
            if let Row::Text(text) = row {
                let value = std::mem::take(text);
                let input = cx.new(|cx| InputState::new(window, cx).default_value(value));
                let subscription = subscribe_input(&input, self.key.clone(), window, cx);
                *row = Row::Input {
                    input,
                    _subscription: subscription,
                };
            }
        }
    }

    pub(super) fn add(&mut self, window: &mut Window, cx: &mut Context<ParameterForm>) {
        self.rows.push(Row::Text(String::new()));
        self.show_page(self.pages() - 1, window, cx);
    }

    pub(super) fn remove(
        &mut self,
        row: usize,
        window: &mut Window,
        cx: &mut Context<ParameterForm>,
    ) {
        if row < self.len() {
            self.rows.remove(row);
            self.show_page(self.page, window, cx);
        }
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
            self.show_page(target / PAGE_SIZE, window, cx);
        }
    }

    pub(in crate::workbench::parameters) fn owns_input(&self, id: EntityId) -> bool {
        self.rows
            .iter()
            .any(|row| matches!(row, Row::Input { input, .. } if input.entity_id() == id))
    }

    pub(in crate::workbench::parameters) fn value(&self, cx: &App) -> Result<Value, String> {
        self.rows
            .iter()
            .map(|row| {
                let text = match row {
                    Row::Text(text) => text.clone(),
                    Row::Input { input, .. } => input.read(cx).value().to_string(),
                };
                if self.numeric {
                    super::super::controls::number(&text)
                } else {
                    Ok(Value::String(text))
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array)
    }
}
