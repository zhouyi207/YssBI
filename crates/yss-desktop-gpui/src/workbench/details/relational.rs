//! Column and predicate drafts use options and restrictions from GraphSemanticSnapshot.
mod columns;
mod predicate;
use super::DetailsPanel;
use gpui::{AnyElement, AppContext, Context, Entity, Window};
use gpui_component::input::InputState;
use serde_json::Value;
use yss_data_contract::{DecimalLiteral, FilterLiteral};
use yss_graph_editor::projection::{EditorFilterLiteralType, EditorParameterConfiguration};
use yss_node_protocol::dataframe::{FilterOperator, FilterPredicate};

pub(super) enum RelationalDraft {
    Columns {
        selected: Vec<String>,
        manual: Vec<Entity<InputState>>,
    },
    Filter(FilterDraft),
}

pub(super) struct FilterDraft {
    column: Entity<InputState>,
    operator: Option<FilterOperator>,
    literal_type: EditorFilterLiteralType,
    input: Entity<InputState>,
}

impl RelationalDraft {
    pub fn new(
        configuration: &EditorParameterConfiguration,
        window: &mut Window,
        cx: &mut Context<DetailsPanel>,
    ) -> Self {
        match configuration {
            EditorParameterConfiguration::ProjectColumns { value, .. } => Self::Columns {
                selected: value.iter().map(ToString::to_string).collect(),
                manual: value
                    .iter()
                    .map(|value| {
                        cx.new(|cx| InputState::new(window, cx).default_value(value.to_string()))
                    })
                    .collect(),
            },
            EditorParameterConfiguration::FilterPredicate { value, .. } => {
                let column = value
                    .as_ref()
                    .and_then(|value| value.get("column"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let operator = value
                    .as_ref()
                    .and_then(|value| value.get("operator"))
                    .and_then(|value| serde_json::from_value(value.clone()).ok());
                let literal = value
                    .as_ref()
                    .and_then(|value| value.get("value"))
                    .and_then(|value| serde_json::from_value::<FilterLiteral>(value.clone()).ok());
                let (literal_type, text) = match literal {
                    Some(FilterLiteral::Boolean(value)) => {
                        (EditorFilterLiteralType::Boolean, value.to_string())
                    }
                    Some(FilterLiteral::Integer(value)) => {
                        (EditorFilterLiteralType::Integer, value.to_string())
                    }
                    Some(FilterLiteral::Decimal(value)) => {
                        (EditorFilterLiteralType::Decimal, value.as_str().to_owned())
                    }
                    Some(FilterLiteral::String(value)) => {
                        (EditorFilterLiteralType::String, value.to_string())
                    }
                    None => (EditorFilterLiteralType::String, String::new()),
                };
                Self::Filter(FilterDraft {
                    column: cx
                        .new(|cx| InputState::new(window, cx).default_value(column.to_owned())),
                    operator,
                    literal_type,
                    input: cx.new(|cx| InputState::new(window, cx).default_value(text)),
                })
            }
            EditorParameterConfiguration::SelectOptions { .. } => {
                unreachable!("select has its own native control")
            }
        }
    }

    pub fn value(&self, cx: &gpui::App) -> Result<Value, String> {
        match self {
            Self::Columns { selected, .. } => Ok(serde_json::json!(selected)),
            Self::Filter(draft) => {
                let column = draft.column.read(cx).value();
                let operator = draft.operator.ok_or("请选择比较方式")?;
                if !yss_data_contract::TabularColumnName::is_valid(&column) {
                    return Err("请选择或输入列名".into());
                }
                let text = draft.input.read(cx).value();
                let value = if operator.requires_value() {
                    Some(match draft.literal_type {
                        EditorFilterLiteralType::Boolean => FilterLiteral::Boolean(text == "true"),
                        EditorFilterLiteralType::Integer => FilterLiteral::Integer(
                            text.trim().parse().map_err(|_| "请输入有效的整数")?,
                        ),
                        EditorFilterLiteralType::Decimal => FilterLiteral::Decimal(
                            DecimalLiteral::new(text.trim()).map_err(|_| "请输入有效的小数")?,
                        ),
                        EditorFilterLiteralType::String => {
                            FilterLiteral::String(text.to_string().into())
                        }
                    })
                } else {
                    None
                };
                serde_json::to_value(FilterPredicate {
                    column: column.to_string().into(),
                    operator,
                    value,
                })
                .map_err(|_| "筛选条件无法提交".into())
            }
        }
    }
}

impl DetailsPanel {
    pub(super) fn render_relational(
        &self,
        index: usize,
        draft: &RelationalDraft,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match draft {
            RelationalDraft::Columns { selected, manual } => {
                self.render_columns(index, selected, manual, busy, cx)
            }
            RelationalDraft::Filter(draft) => self.render_filter(index, draft, busy, cx),
        }
    }
}
