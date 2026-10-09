//! Typed predicate state uses issued options, never display labels, as identities.
use crate::{
    text::translate,
    workbench::parameters::{ParameterForm, field::subscribe_input},
};
use gpui::{App, AppContext, Context, Entity, EntityId, Subscription, Window};
use gpui_component::input::InputState;
use serde_json::Value;
use yss_data_contract::{DecimalLiteral, FilterLiteral, TabularColumnName};
use yss_graph_editor::projection::{
    EditorFilterColumnOption, EditorFilterLiteralType as LiteralType,
    EditorParameterConfiguration as Configuration, EditorParameterModel,
};
use yss_node_protocol::dataframe::{FilterOperator, FilterPredicate};

const OPERATORS: &[FilterOperator] = &[
    FilterOperator::Equal,
    FilterOperator::NotEqual,
    FilterOperator::LessThan,
    FilterOperator::LessThanOrEqual,
    FilterOperator::GreaterThan,
    FilterOperator::GreaterThanOrEqual,
    FilterOperator::IsNull,
    FilterOperator::IsNotNull,
];
const LITERAL_TYPES: &[LiteralType] = &[
    LiteralType::Boolean,
    LiteralType::Integer,
    LiteralType::Decimal,
    LiteralType::String,
];

pub(in crate::workbench::parameters) struct FilterDraft {
    pub(super) column: Entity<InputState>,
    pub(super) operator: Option<FilterOperator>,
    pub(super) literal_type: LiteralType,
    pub(super) input: Entity<InputState>,
    _subscriptions: [Subscription; 2],
}

impl FilterDraft {
    pub(in crate::workbench::parameters::relational) fn new(
        model: &EditorParameterModel,
        window: &mut Window,
        cx: &mut Context<ParameterForm>,
    ) -> Self {
        let Some(Configuration::FilterPredicate { value, columns, .. }) = &model.configuration
        else {
            unreachable!()
        };
        let column = value
            .as_ref()
            .and_then(|value| value.get("column"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let selected = columns.iter().find(|option| option.name.as_ref() == column);
        let operator = value
            .as_ref()
            .and_then(|value| value.get("operator"))
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .or_else(|| selected.and_then(|option| option.operators.first().copied()))
            .or(Some(FilterOperator::Equal));
        let literal = value
            .as_ref()
            .and_then(|value| value.get("value"))
            .and_then(|value| serde_json::from_value::<FilterLiteral>(value.clone()).ok());
        let (literal_type, text) = match literal {
            Some(FilterLiteral::Boolean(value)) => (LiteralType::Boolean, value.to_string()),
            Some(FilterLiteral::Integer(value)) => (LiteralType::Integer, value.to_string()),
            Some(FilterLiteral::Decimal(value)) => {
                (LiteralType::Decimal, value.as_str().to_owned())
            }
            Some(FilterLiteral::String(value)) => (LiteralType::String, value.to_string()),
            None => {
                let kind = selected
                    .and_then(|option| option.literal_types.first())
                    .copied()
                    .unwrap_or(LiteralType::String);
                (kind, initial_value(kind).into())
            }
        };
        let column = cx.new(|cx| InputState::new(window, cx).default_value(column.to_owned()));
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
        let subscriptions = [
            subscribe_input(&column, model.key.clone(), window, cx),
            subscribe_input(&input, model.key.clone(), window, cx),
        ];
        Self {
            column,
            operator,
            literal_type,
            input,
            _subscriptions: subscriptions,
        }
    }

    pub(super) fn selected<'a>(
        &self,
        configuration: &'a Configuration,
        cx: &App,
    ) -> Option<&'a EditorFilterColumnOption> {
        let Configuration::FilterPredicate { columns, .. } = configuration else {
            return None;
        };
        let name = self.column.read(cx).value();
        columns
            .iter()
            .find(|option| option.name.as_ref() == name.as_ref())
    }

    pub(super) fn operators<'a>(
        &self,
        configuration: &'a Configuration,
        cx: &App,
    ) -> &'a [FilterOperator] {
        if matches!(
            configuration,
            Configuration::FilterPredicate {
                schema_known: false,
                ..
            }
        ) {
            OPERATORS
        } else {
            self.selected(configuration, cx)
                .map(|column| column.operators.as_ref())
                .unwrap_or_default()
        }
    }

    pub(super) fn literal_types<'a>(
        &self,
        configuration: &'a Configuration,
        cx: &App,
    ) -> &'a [LiteralType] {
        if matches!(
            configuration,
            Configuration::FilterPredicate {
                schema_known: false,
                ..
            }
        ) {
            LITERAL_TYPES
        } else {
            self.selected(configuration, cx)
                .map(|column| column.literal_types.as_ref())
                .unwrap_or_default()
        }
    }

    pub(super) fn choose_column(
        &mut self,
        name: &str,
        configuration: &Configuration,
        window: &mut Window,
        cx: &mut Context<ParameterForm>,
    ) -> bool {
        if self.column.read(cx).value() == name {
            return false;
        }
        let Configuration::FilterPredicate { columns, .. } = configuration else {
            return false;
        };
        let Some(option) = columns.iter().find(|option| option.name.as_ref() == name) else {
            return false;
        };
        self.column
            .update(cx, |input, cx| input.set_value(name.to_owned(), window, cx));
        if self
            .operator
            .is_none_or(|operator| !option.operators.contains(&operator))
        {
            self.operator = option.operators.first().copied();
        }
        if !option.literal_types.contains(&self.literal_type)
            && let Some(kind) = option.literal_types.first()
        {
            self.choose_type(*kind, window, cx);
        }
        true
    }

    pub(super) fn choose_type(
        &mut self,
        kind: LiteralType,
        window: &mut Window,
        cx: &mut Context<ParameterForm>,
    ) -> bool {
        if self.literal_type == kind {
            return false;
        }
        self.literal_type = kind;
        self.input.update(cx, |input, cx| {
            input.set_value(initial_value(kind), window, cx)
        });
        true
    }

    pub(in crate::workbench::parameters::relational) fn owns_input(&self, id: EntityId) -> bool {
        self.column.entity_id() == id || self.input.entity_id() == id
    }

    pub(in crate::workbench::parameters::relational) fn value(
        &self,
        configuration: &Configuration,
        cx: &App,
    ) -> Result<Value, String> {
        let column = self.column.read(cx).value();
        if !TabularColumnName::is_valid(&column) {
            return Err(translate("native.workbench.chooseColumn"));
        }
        let operator = self
            .operator
            .ok_or_else(|| translate("native.workbench.chooseComparison"))?;
        if !self.operators(configuration, cx).contains(&operator) {
            return Err(translate("detail.parameterEditor.unavailableChoice"));
        }
        let value = if operator.requires_value() {
            if !self
                .literal_types(configuration, cx)
                .contains(&self.literal_type)
            {
                return Err(translate("detail.parameterEditor.unavailableChoice"));
            }
            let text = self.input.read(cx).value();
            Some(match self.literal_type {
                LiteralType::Boolean => FilterLiteral::Boolean(text == "true"),
                LiteralType::Integer => FilterLiteral::Integer(
                    text.trim()
                        .parse()
                        .map_err(|_| translate("native.workbench.invalidInteger"))?,
                ),
                LiteralType::Decimal => FilterLiteral::Decimal(
                    DecimalLiteral::new(text.trim())
                        .map_err(|_| translate("native.workbench.invalidDecimal"))?,
                ),
                LiteralType::String => FilterLiteral::String(text.to_string().into()),
            })
        } else {
            None
        };
        serde_json::to_value(FilterPredicate {
            column: column.to_string().into(),
            operator,
            value,
        })
        .map_err(|_| translate("native.workbench.filterSubmitFailed"))
    }
}

fn initial_value(kind: LiteralType) -> &'static str {
    if kind == LiteralType::Boolean {
        "false"
    } else {
        ""
    }
}
