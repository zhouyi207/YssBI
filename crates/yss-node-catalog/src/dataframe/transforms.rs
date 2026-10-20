//! Native-plan data transformations and their authoring contracts.
use super::*;

#[derive(Clone, Copy)]
enum Kind {
    Impute,
    Sort,
    Deduplicate,
    SetColumn,
    Mask,
    Unpivot,
    Pivot,
    Resample,
    Choose,
    IsNull,
    Fill,
    Map,
    Trim,
    Lower,
    Upper,
    Replace,
    Substring,
    Split,
    Concatenate,
    DatePart,
    DateTruncate,
    DateAdd,
    DateDifference,
    Clip,
    Bin,
    Cumulative,
    Rank,
    ForwardFill,
    BackwardFill,
    Encode,
}

impl Kind {
    fn is_frame(self) -> bool {
        matches!(
            self,
            Self::Sort
                | Self::Deduplicate
                | Self::SetColumn
                | Self::Mask
                | Self::Unpivot
                | Self::Pivot
                | Self::Resample
        )
    }
}

struct Entry {
    id: &'static str,
    kind: Kind,
}
macro_rules! entry {
    ($id:literal, $kind:ident) => {
        Entry {
            id: $id,
            kind: Kind::$kind,
        }
    };
}
const ENTRIES: &[Entry] = &[
    entry!("yssbi.dataframe.impute.single", Impute),
    entry!("yssbi.dataframe.sort", Sort),
    entry!("yssbi.dataframe.deduplicate", Deduplicate),
    entry!("yssbi.dataframe.set_column", SetColumn),
    entry!("yssbi.dataframe.filter.mask", Mask),
    entry!("yssbi.dataframe.unpivot", Unpivot),
    entry!("yssbi.dataframe.pivot", Pivot),
    entry!("yssbi.dataframe.resample", Resample),
    entry!("yssbi.dataframe.series.choose", Choose),
    entry!("yssbi.dataframe.series.is_null", IsNull),
    entry!("yssbi.dataframe.series.is_not_null", IsNull),
    entry!("yssbi.dataframe.series.fill_null", Fill),
    entry!("yssbi.dataframe.series.map", Map),
    entry!("yssbi.dataframe.series.text.trim", Trim),
    entry!("yssbi.dataframe.series.text.lower", Lower),
    entry!("yssbi.dataframe.series.text.upper", Upper),
    entry!("yssbi.dataframe.series.text.replace", Replace),
    entry!("yssbi.dataframe.series.text.substring", Substring),
    entry!("yssbi.dataframe.series.text.split", Split),
    entry!("yssbi.dataframe.series.text.concatenate", Concatenate),
    entry!("yssbi.dataframe.series.datetime.part", DatePart),
    entry!("yssbi.dataframe.series.datetime.truncate", DateTruncate),
    entry!("yssbi.dataframe.series.datetime.add", DateAdd),
    entry!("yssbi.dataframe.series.datetime.difference", DateDifference),
    entry!("yssbi.dataframe.series.clip", Clip),
    entry!("yssbi.dataframe.series.bin", Bin),
    entry!("yssbi.dataframe.series.cumulative", Cumulative),
    entry!("yssbi.dataframe.series.rank", Rank),
    entry!("yssbi.dataframe.series.forward_fill", ForwardFill),
    entry!("yssbi.dataframe.series.backward_fill", BackwardFill),
    entry!("yssbi.dataframe.encode", Encode),
];

pub(super) const SCHEMA_RESOLVER: &str = "yssbi.dataframe.schema.transform";
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    fragment
        .schema_resolvers
        .push(sid(SCHEMA_RESOLVER, SchemaResolverId::new)?);
    for entry in ENTRIES {
        let (ports, mut parameters, generic) = interface(entry.kind)?;
        node_scoped_keys(entry.id, &mut parameters)?;
        let frame = entry.kind.is_frame();
        let protocol = NodeProtocol {
            type_id: sid(entry.id, NodeTypeId::new)?,
            catalog: NodeCatalogProtocol {
                title_key: node_key(entry.id, "title")?,
                aliases_key: Some(node_key(entry.id, "aliases")?),
                category_id: sid(
                    if matches!(entry.kind, Kind::Impute) {
                        "statistics.imputation"
                    } else if frame {
                        "dataframe"
                    } else {
                        "dataframe.series"
                    },
                    NodeCategoryId::new,
                )?,
                icon_id: sid("builtin.dataframe", IconId::new)?,
                style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(
                entry.id,
                ports,
                if generic {
                    vec![sid("element", TypeParameterId::new)?]
                } else {
                    vec![]
                },
                vec![],
            )?,
            parameters: assembled_parameters(entry.id, parameters)?,
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: pure(),
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        };
        fragment.nodes.push(leaf(protocol, entry.id));
    }
    Ok(())
}

fn list(key: &'static str, element: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    let value_type = data_series_type(concrete(element)?);
    parameter(
        key,
        value_type.clone(),
        ParameterEditorSpec::Auto,
        Some(TypedValue {
            value_type,
            value: DataValue::List(vec![]),
        }),
        vec![],
    )
}
fn columns(key: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    let mut parameter = list(key, "core.text")?;
    parameter.constraints.push(ParameterConstraint::ColumnNames);
    Ok(parameter)
}

fn toggle(key: &'static str, value: bool) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.binary")?,
        ParameterEditorSpec::Toggle,
        Some(TypedValue {
            value_type: concrete("core.binary")?,
            value: DataValue::Bool(value),
        }),
        vec![],
    )
}
fn text_value(key: &'static str, value: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.text")?,
        ParameterEditorSpec::Text { multiline: false },
        Some(TypedValue {
            value_type: concrete("core.text")?,
            value: DataValue::String(value.into()),
        }),
        vec![],
    )
}
fn number(
    key: &'static str,
    value: i64,
    min: Option<i64>,
) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.numeric")?,
        ParameterEditorSpec::Number,
        Some(TypedValue {
            value_type: concrete("core.numeric")?,
            value: DataValue::Integer(value),
        }),
        vec![ParameterConstraint::IntegerRange { min, max: None }],
    )
}
pub(super) fn offset_parameter() -> Result<Parameter, BuiltinAssemblyError> {
    number("offset", 0, Some(0))
}
pub(super) fn minimum_periods_parameter() -> Result<Parameter, BuiltinAssemblyError> {
    number("min_periods", 0, Some(0))
}
pub(super) fn window_context() -> Result<PortSpec, BuiltinAssemblyError> {
    let mut context = user_input("context", "Context", dataframe_type()?, 0)?;
    context.cardinality = PortCardinality::UserCreated {
        min: 0,
        max: Some(1),
    };
    Ok(context)
}
pub(super) fn window_parameters() -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    Ok(vec![
        columns("partition_by")?,
        columns("order_by")?,
        toggle("descending", false)?,
        toggle("nulls_first", false)?,
    ])
}
fn mixed(element: TypeExpr) -> TypeExpr {
    let mut members = if matches!(element, TypeExpr::Generic(_)) {
        yss_data_contract::SemanticType::ALL
            .into_iter()
            .map(|semantic| {
                TypeExpr::Concrete(TypeId::new(semantic.type_id()).expect("semantic type id"))
            })
            .collect::<Vec<_>>()
    } else {
        vec![element.clone()]
    };
    members.push(data_series_type(element));
    TypeExpr::Union(members)
}
fn interface(kind: Kind) -> Result<(Vec<PortSpec>, Vec<Parameter>, bool), BuiltinAssemblyError> {
    use Kind::*;
    let generic = matches!(
        kind,
        SetColumn | Choose | IsNull | Fill | Map | ForwardFill | BackwardFill
    );
    let element = TypeExpr::Generic(sid("element", TypeParameterId::new)?);
    let generic_series = data_series_type(element.clone());
    let frame = kind.is_frame();
    let mut parameters = Vec::new();
    let mut ports = if frame {
        relational_ports(derived_schema(
            SCHEMA_RESOLVER,
            vec![SchemaDependency::Port(port_key("source")?)],
        )?)?
    } else {
        let (input, output) = match kind {
            Choose => (
                data_series_type(concrete("core.binary")?),
                generic_series.clone(),
            ),
            IsNull => (
                generic_series.clone(),
                data_series_type(concrete("core.binary")?),
            ),
            Fill | Map | ForwardFill | BackwardFill => {
                (generic_series.clone(), generic_series.clone())
            }
            Trim | Lower | Upper | Replace | Substring | Split | Concatenate => (
                data_series_type(concrete("core.text")?),
                data_series_type(concrete("core.text")?),
            ),
            DatePart | DateDifference => (
                data_series_type(concrete("core.datetime")?),
                numeric_data_series_type(),
            ),
            DateTruncate | DateAdd => (
                data_series_type(concrete("core.datetime")?),
                data_series_type(concrete("core.datetime")?),
            ),
            Bin => (
                numeric_data_series_type(),
                data_series_type(concrete("core.ordinal")?),
            ),
            Rank => (
                TypeExpr::Union(
                    ["core.numeric", "core.text", "core.datetime", "core.ordinal"]
                        .into_iter()
                        .map(|id| concrete(id).map(data_series_type))
                        .collect::<Result<_, _>>()?,
                ),
                numeric_data_series_type(),
            ),
            Encode => (
                TypeExpr::Union(
                    ["core.categorical", "core.ordinal", "core.binary"]
                        .into_iter()
                        .map(|id| concrete(id).map(data_series_type))
                        .collect::<Result<_, _>>()?,
                ),
                dataframe_type()?,
            ),
            _ => (numeric_data_series_type(), numeric_data_series_type()),
        };
        vec![
            data_input(
                if matches!(kind, Choose) {
                    "condition"
                } else {
                    "series"
                },
                "Series",
                input,
            )?,
            data_output(
                "result",
                "Result",
                output,
                if matches!(kind, Encode) {
                    Some(derived_schema(
                        SCHEMA_RESOLVER,
                        vec![SchemaDependency::Port(port_key("series")?)],
                    )?)
                } else {
                    None
                },
            )?,
        ]
    };
    match kind {
        Impute => {
            parameters = vec![
                choice_parameter(
                    "imputation_method",
                    "mean",
                    &["mean", "median", "mode", "constant"],
                )?,
                parameter(
                    "fill_value",
                    concrete("core.numeric")?,
                    ParameterEditorSpec::Number,
                    Some(TypedValue {
                        value_type: concrete("core.numeric")?,
                        value: DataValue::Integer(0),
                    }),
                    vec![],
                )?,
            ];
        }
        Sort => {
            parameters = vec![
                nominal_parameter(
                    "columns",
                    yss_node_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
                )?,
                columns("descending_columns")?,
                toggle("nulls_first", false)?,
            ];
        }
        Deduplicate => {
            parameters = vec![
                columns("keys")?,
                choice_parameter("keep", "first", &["first", "last", "none"])?,
            ];
        }
        SetColumn => {
            ports.insert(1, data_input("series", "Series", generic_series)?);
            parameters = vec![text_value("name", "computed")?];
        }
        Mask => {
            ports.insert(
                1,
                data_input("mask", "Mask", data_series_type(concrete("core.binary")?))?,
            );
            parameters = vec![toggle("drop_matches", false)?];
        }
        Unpivot => {
            parameters = vec![
                columns("keys")?,
                nominal_parameter(
                    "columns",
                    yss_node_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
                )?,
                text_value("variable_name", "variable")?,
                text_value("value_name", "value")?,
                toggle("include_null", true)?,
            ];
        }
        Pivot => {
            parameters = vec![
                columns("keys")?,
                column_parameter("category_column")?,
                column_parameter("value_column")?,
                list("levels", "core.text")?,
                columns("names")?,
                choice_parameter("aggregate", "sum", &["sum", "mean", "min", "max", "count"])?,
            ];
        }
        Resample => {
            parameters = vec![
                column_parameter("time_column")?,
                choice_parameter(
                    "unit",
                    "day",
                    &[
                        "year", "quarter", "month", "week", "day", "hour", "minute", "second",
                    ],
                )?,
                columns("keys")?,
                nominal_parameter(
                    "columns",
                    yss_node_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
                )?,
                choice_parameter("aggregate", "mean", &["sum", "mean", "min", "max", "count"])?,
            ];
        }
        Choose => {
            ports.insert(
                1,
                data_input("when_true", "When True", mixed(element.clone()))?,
            );
            ports.insert(2, data_input("when_false", "When False", mixed(element))?);
        }
        Fill => {
            ports.insert(1, data_input("replacement", "Replacement", mixed(element))?);
        }
        Map => {
            parameters = vec![
                list("from_values", "core.text")?,
                list("to_values", "core.text")?,
                toggle("keep_unmatched", true)?,
            ];
        }
        Replace => {
            parameters = vec![required_text_parameter("from")?, text_value("to", "")?];
        }
        Substring => {
            parameters = vec![number("start", 1, Some(1))?, number("length", 10, Some(0))?];
        }
        Split => {
            parameters = vec![text_value("separator", ",")?, number("part", 1, None)?];
        }
        Concatenate => {
            ports[0] = user_input("parts", "Text", mixed(concrete("core.text")?), 2)?;
            parameters = vec![text_value("separator", "")?];
        }
        DatePart => {
            parameters = vec![choice_parameter(
                "part",
                "year",
                &[
                    "year", "quarter", "month", "week", "day", "doy", "dow", "hour", "minute",
                    "second",
                ],
            )?];
        }
        DateTruncate => {
            parameters = vec![choice_parameter(
                "unit",
                "day",
                &[
                    "year", "quarter", "month", "week", "day", "hour", "minute", "second",
                ],
            )?];
        }
        DateAdd => {
            parameters = vec![
                choice_parameter(
                    "unit",
                    "day",
                    &[
                        "year",
                        "month",
                        "week",
                        "day",
                        "hour",
                        "minute",
                        "second",
                        "millisecond",
                        "microsecond",
                        "nanosecond",
                    ],
                )?,
                number("amount", 1, None)?,
            ];
        }
        DateDifference => {
            ports.insert(
                1,
                data_input("other", "Other", mixed(concrete("core.datetime")?))?,
            );
            parameters = vec![choice_parameter(
                "unit",
                "day",
                &[
                    "week",
                    "day",
                    "hour",
                    "minute",
                    "second",
                    "millisecond",
                    "microsecond",
                    "nanosecond",
                ],
            )?];
        }
        Clip => {
            parameters = vec![number("lower", 0, None)?, number("upper", 1, None)?];
            for parameter in &mut parameters {
                parameter.constraints = vec![];
            }
        }
        Bin => {
            parameters = vec![list("edges", "core.numeric")?, list("labels", "core.text")?];
        }
        Cumulative | Rank | ForwardFill | BackwardFill => {
            ports.insert(1, window_context()?);
            parameters = window_parameters()?;
            if matches!(kind, Cumulative) {
                parameters.insert(
                    0,
                    choice_parameter("operation", "sum", &["sum", "mean", "min", "max", "std"])?,
                );
            }
            if matches!(kind, Rank) {
                parameters.insert(0, toggle("dense", false)?);
            }
        }
        Encode => {
            parameters = vec![
                list("levels", "core.text")?,
                columns("names")?,
                toggle("drop_reference", false)?,
                text_value("reference", "")?,
            ];
        }
        _ => {}
    }
    Ok((ports, parameters, generic))
}
