//! Data-preparation adapters reuse native plans, binding independent operands by position.
use super::{numeric_input, relational::kernel_error, series::SeriesKernel};
use crate::{
    KernelContract, KernelError, KernelId, KernelInputSpec as Input, KernelInvocation,
    KernelParameterKey, KernelRegistryBuilder, RuntimeValue,
};
use std::sync::Arc;
use yss_data_contract::{SemanticType, TabularScalar, ValueType};
use yss_relational_contract::*;

#[derive(Clone, Copy)]
enum Operation {
    Impute,
    Sort,
    Deduplicate,
    SetColumn,
    Mask,
    Unpivot,
    Pivot,
    Resample,
    Choose,
    IsNull(bool),
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

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    use Operation::*;
    let window_parameters = &["partition_by", "order_by", "descending", "nulls_first"][..];
    let entries: &[(&str, Operation, &[&str], &[&str])] = &[
        (
            "yssbi.dataframe.impute.single",
            Impute,
            &["series"],
            &["imputation_method", "fill_value"],
        ),
        (
            "yssbi.dataframe.sort",
            Sort,
            &["source"],
            &["columns", "descending_columns", "nulls_first"],
        ),
        (
            "yssbi.dataframe.deduplicate",
            Deduplicate,
            &["source"],
            &["keys", "keep"],
        ),
        (
            "yssbi.dataframe.set_column",
            SetColumn,
            &["source", "series"],
            &["name"],
        ),
        (
            "yssbi.dataframe.filter.mask",
            Mask,
            &["source", "mask"],
            &["drop_matches"],
        ),
        (
            "yssbi.dataframe.unpivot",
            Unpivot,
            &["source"],
            &[
                "keys",
                "columns",
                "variable_name",
                "value_name",
                "include_null",
            ],
        ),
        (
            "yssbi.dataframe.pivot",
            Pivot,
            &["source"],
            &[
                "keys",
                "category_column",
                "value_column",
                "levels",
                "names",
                "aggregate",
            ],
        ),
        (
            "yssbi.dataframe.resample",
            Resample,
            &["source"],
            &["time_column", "unit", "keys", "columns", "aggregate"],
        ),
        (
            "yssbi.dataframe.series.choose",
            Choose,
            &["condition", "when_true", "when_false"],
            &[],
        ),
        (
            "yssbi.dataframe.series.is_null",
            IsNull(false),
            &["series"],
            &[],
        ),
        (
            "yssbi.dataframe.series.is_not_null",
            IsNull(true),
            &["series"],
            &[],
        ),
        (
            "yssbi.dataframe.series.fill_null",
            Fill,
            &["series", "replacement"],
            &[],
        ),
        (
            "yssbi.dataframe.series.map",
            Map,
            &["series"],
            &["from_values", "to_values", "keep_unmatched"],
        ),
        ("yssbi.dataframe.series.text.trim", Trim, &["series"], &[]),
        ("yssbi.dataframe.series.text.lower", Lower, &["series"], &[]),
        ("yssbi.dataframe.series.text.upper", Upper, &["series"], &[]),
        (
            "yssbi.dataframe.series.text.replace",
            Replace,
            &["series"],
            &["from", "to"],
        ),
        (
            "yssbi.dataframe.series.text.substring",
            Substring,
            &["series"],
            &["start", "length"],
        ),
        (
            "yssbi.dataframe.series.text.split",
            Split,
            &["series"],
            &["separator", "part"],
        ),
        (
            "yssbi.dataframe.series.text.concatenate",
            Concatenate,
            &["parts"],
            &["separator"],
        ),
        (
            "yssbi.dataframe.series.datetime.part",
            DatePart,
            &["series"],
            &["part"],
        ),
        (
            "yssbi.dataframe.series.datetime.truncate",
            DateTruncate,
            &["series"],
            &["unit"],
        ),
        (
            "yssbi.dataframe.series.datetime.add",
            DateAdd,
            &["series"],
            &["unit", "amount"],
        ),
        (
            "yssbi.dataframe.series.datetime.difference",
            DateDifference,
            &["series", "other"],
            &["unit"],
        ),
        (
            "yssbi.dataframe.series.clip",
            Clip,
            &["series"],
            &["lower", "upper"],
        ),
        (
            "yssbi.dataframe.series.bin",
            Bin,
            &["series"],
            &["edges", "labels"],
        ),
        (
            "yssbi.dataframe.series.cumulative",
            Cumulative,
            &["series", "context"],
            &[
                "operation",
                "partition_by",
                "order_by",
                "descending",
                "nulls_first",
            ],
        ),
        (
            "yssbi.dataframe.series.rank",
            Rank,
            &["series", "context"],
            &[
                "dense",
                "partition_by",
                "order_by",
                "descending",
                "nulls_first",
            ],
        ),
        (
            "yssbi.dataframe.series.forward_fill",
            ForwardFill,
            &["series", "context"],
            window_parameters,
        ),
        (
            "yssbi.dataframe.series.backward_fill",
            BackwardFill,
            &["series", "context"],
            window_parameters,
        ),
        (
            "yssbi.dataframe.encode",
            Encode,
            &["series"],
            &["levels", "names", "drop_reference", "reference"],
        ),
    ];
    for &(id, operation, keys, parameters) in entries {
        let inputs = keys
            .iter()
            .map(|key| match *key {
                "context" => Input::repeated("context", 0..=1),
                "parts" => Input::repeated("parts", 2..=usize::MAX),
                key => Input::fixed(key),
            })
            .collect::<Vec<_>>();
        builder
            .register(
                KernelId::new(id.into()).expect("kernel id"),
                std::num::NonZeroU32::new(2).unwrap(),
                KernelContract::new(
                    inputs,
                    parameters
                        .iter()
                        .map(|key| KernelParameterKey::new((*key).into()).unwrap()),
                    1..=1,
                )
                .unwrap(),
                move |inv| execute(operation, inv),
            )
            .expect("unique transformation kernel");
    }
}

pub(super) fn text<'a>(inv: &'a KernelInvocation<'_>, key: &str) -> Result<&'a str, KernelError> {
    match inv.parameter(key).map(RuntimeValue::unannotated) {
        Some(RuntimeValue::Scalar(TabularScalar::String(s))) => Ok(s),
        _ => Err(KernelError::InvalidParameter),
    }
}
pub(super) fn integer(inv: &KernelInvocation<'_>, key: &str) -> Result<i64, KernelError> {
    match inv.parameter(key).map(RuntimeValue::unannotated) {
        Some(RuntimeValue::Scalar(TabularScalar::Integer(v))) => Ok(*v),
        Some(RuntimeValue::Scalar(TabularScalar::Unsigned(v))) => {
            i64::try_from(*v).map_err(|_| KernelError::InvalidParameter)
        }
        _ => Err(KernelError::InvalidParameter),
    }
}
fn positive(inv: &KernelInvocation<'_>, key: &str) -> Result<usize, KernelError> {
    usize::try_from(integer(inv, key)?)
        .ok()
        .filter(|v| *v > 0)
        .ok_or(KernelError::InvalidParameter)
}
fn boolean(inv: &KernelInvocation<'_>, key: &str) -> Result<bool, KernelError> {
    match inv.parameter(key).map(RuntimeValue::unannotated) {
        Some(RuntimeValue::Scalar(TabularScalar::Bool(v))) => Ok(*v),
        _ => Err(KernelError::InvalidParameter),
    }
}
fn strings(inv: &KernelInvocation<'_>, key: &str) -> Result<Vec<Box<str>>, KernelError> {
    match inv.parameter(key).map(RuntimeValue::unannotated) {
        Some(RuntimeValue::List(values)) => values
            .iter()
            .map(|v| match v.unannotated() {
                RuntimeValue::Scalar(TabularScalar::String(s)) => Ok(s.clone()),
                _ => Err(KernelError::InvalidParameter),
            })
            .collect(),
        _ => Err(KernelError::InvalidParameter),
    }
}
fn values(inv: &KernelInvocation<'_>, key: &str) -> Result<Vec<TabularScalar>, KernelError> {
    match inv.parameter(key).map(RuntimeValue::unannotated) {
        Some(RuntimeValue::List(values)) => values
            .iter()
            .map(|v| {
                v.tabular_scalar()
                    .map_err(|_| KernelError::InvalidParameter)
            })
            .collect(),
        _ => Err(KernelError::InvalidParameter),
    }
}
fn aggregate(inv: &KernelInvocation<'_>) -> Result<PivotAggregate, KernelError> {
    Ok(match text(inv, "aggregate")? {
        "sum" => PivotAggregate::Sum,
        "mean" => PivotAggregate::Mean,
        "min" => PivotAggregate::Min,
        "max" => PivotAggregate::Max,
        "count" => PivotAggregate::Count,
        _ => return Err(KernelError::InvalidParameter),
    })
}
fn window_operation(inv: &KernelInvocation<'_>) -> Result<WindowOperation, KernelError> {
    Ok(match text(inv, "operation")? {
        "sum" => WindowOperation::Sum,
        "mean" => WindowOperation::Mean,
        "min" => WindowOperation::Min,
        "max" => WindowOperation::Max,
        "std" => WindowOperation::StandardDeviation,
        _ => return Err(KernelError::InvalidParameter),
    })
}
fn levels(inv: &KernelInvocation<'_>) -> Result<Vec<PivotLevel>, KernelError> {
    let values = strings(inv, "levels")?;
    let names = strings(inv, "names")?;
    if values.is_empty() || values.len() != names.len() {
        return Err(KernelError::InvalidParameter);
    }
    Ok(values
        .into_iter()
        .zip(names)
        .map(|(value, name)| PivotLevel {
            value: TabularScalar::String(value),
            name,
        })
        .collect())
}
fn context(inv: &KernelInvocation<'_>) -> Result<Option<RelationHandle>, KernelError> {
    inv.inputs
        .iter()
        .zip(inv.input_keys)
        .find(|(_, key)| **key == "context")
        .map(|(value, _)| match value {
            RuntimeValue::Relation(r) => Ok(r.clone()),
            _ => Err(KernelError::InvalidParameter),
        })
        .transpose()
}
fn window(inv: &KernelInvocation<'_>) -> Result<SeriesWindow, KernelError> {
    Ok(SeriesWindow {
        context: context(inv)?,
        partition_by: strings(inv, "partition_by")?,
        order_by: strings(inv, "order_by")?
            .into_iter()
            .map(|column| {
                Ok(SortColumn {
                    column,
                    ascending: !boolean(inv, "descending")?,
                    nulls_first: boolean(inv, "nulls_first")?,
                })
            })
            .collect::<Result<_, KernelError>>()?,
        require_unique_keys: false,
    })
}
fn semantic(value: &RuntimeValue) -> SemanticType {
    value
        .metadata()
        .map(|m| m.semantic.kind)
        .unwrap_or_else(|| match value.unannotated() {
            RuntimeValue::List(values) => values
                .iter()
                .find_map(|v| match v.unannotated() {
                    RuntimeValue::Scalar(TabularScalar::Null) => None,
                    RuntimeValue::Scalar(TabularScalar::String(_)) => Some(SemanticType::Text),
                    RuntimeValue::Scalar(TabularScalar::Bool(_)) => Some(SemanticType::Binary),
                    _ => Some(SemanticType::Numeric),
                })
                .unwrap_or(SemanticType::Numeric),
            _ => SemanticType::Numeric,
        })
}

/// Literal values already exist in memory; import them once and return expressions thereafter.
pub(super) fn operands(
    inputs: &[&RuntimeValue],
    inv: &KernelInvocation<'_>,
) -> Result<(RelationHandle, Vec<SeriesOperand>), KernelError> {
    let columns = inputs
        .iter()
        .copied()
        .filter(|v| {
            matches!(
                v.unannotated(),
                RuntimeValue::Series(_) | RuntimeValue::List(_)
            )
        })
        .collect::<Vec<_>>();
    let first = columns.first().ok_or(KernelError::InvalidParameter)?;
    let shared = match first.unannotated() {
        RuntimeValue::Series(first) => columns.iter().all(|v| matches!(v.unannotated(), RuntimeValue::Series(h) if first.relation().shares_row_domain(h.relation()))),
        _ => false,
    };
    let relation = if shared {
        let RuntimeValue::Series(first) = first.unannotated() else {
            unreachable!()
        };
        first.relation().clone()
    } else {
        super::series::relation(&columns, inv)?
    };
    let mut index = 0;
    let values = inputs
        .iter()
        .map(|v| match v.unannotated() {
            RuntimeValue::Series(handle) if shared => Ok(SeriesOperand::Series(handle.clone())),
            RuntimeValue::Series(_) | RuntimeValue::List(_) => {
                let series = relation
                    .select_series(&format!("value_{index}"))
                    .map_err(kernel_error)?;
                index += 1;
                Ok(SeriesOperand::Series(series))
            }
            _ => v
                .tabular_scalar()
                .map(SeriesOperand::Scalar)
                .map_err(|_| KernelError::InvalidParameter),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((relation, values))
}

pub(super) fn series_input(
    value: &RuntimeValue,
    inv: &KernelInvocation<'_>,
) -> Result<SeriesHandle, KernelError> {
    if let RuntimeValue::Series(series) = value.unannotated() {
        return Ok(series.clone());
    }
    if let RuntimeValue::List(values) = value.unannotated() {
        let mut bytes = 0usize;
        for (index, value) in values.iter().enumerate() {
            if index % 1024 == 0 {
                inv.check_control()?;
            }
            let size = size_of::<TabularScalar>()
                + match value.unannotated() {
                    RuntimeValue::Scalar(TabularScalar::String(s)) => s.len(),
                    _ => 0,
                };
            bytes = inv.control.check_bytes(bytes.checked_add(size))?;
        }
        let mut scalars = inv.control.reserve(values.len())?;
        for value in values.iter() {
            scalars.push(
                value
                    .tabular_scalar()
                    .map_err(|_| KernelError::InvalidParameter)?,
            );
        }
        let metadata = value
            .metadata()
            .cloned()
            .unwrap_or(yss_data_contract::ConversionMetadata {
                semantic: yss_data_contract::ColumnSemantic::new(semantic(value)),
                temporal: None,
                dummy_base_level: None,
            });
        let (field, array) =
            yss_database_arrow::materialized_column("value", &scalars, Some(&metadata))
                .map_err(|_| KernelError::InvalidParameter)?;
        return Arc::clone(inv.relations)
            .literal_series(field, array, &inv.relation_control())
            .map_err(kernel_error);
    }
    Err(KernelError::InvalidParameter)
}
fn scalar_results(
    relation: &RelationHandle,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<TabularScalar>, KernelError> {
    let mut result = None;
    relation
        .visit_batches(&inv.relation_control(), &mut |batch| {
            if batch.num_rows() == 0 {
                return Ok(());
            }
            if batch.num_rows() != 1 || result.is_some() {
                return Err(RelationError::InvalidPlan);
            }
            result = Some(
                batch
                    .columns()
                    .iter()
                    .map(|array| {
                        // Integer SUM uses a wide decimal accumulator, but its public scalar
                        // remains an exact i64/u64. Avoid the general decimal-to-f64 reader.
                        if matches!(array.data_type(), arrow_schema::DataType::Decimal128(_, 0)) {
                            use arrow_array::Array;
                            if array.is_null(0) {
                                return Ok(TabularScalar::Null);
                            }
                            let number = array
                                .as_any()
                                .downcast_ref::<arrow_array::Decimal128Array>()
                                .ok_or(RelationError::InvalidPlan)?
                                .value(0);
                            return i64::try_from(number)
                                .map(TabularScalar::Integer)
                                .or_else(|_| u64::try_from(number).map(TabularScalar::Unsigned))
                                .map_err(|_| RelationError::NonFiniteResult);
                        }
                        yss_database_arrow::materialized_values(array.as_ref())
                            .map_err(|_| RelationError::NonFiniteResult)
                            .and_then(|mut values| values.pop().ok_or(RelationError::InvalidPlan))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            Ok(())
        })
        .map_err(kernel_error)?;
    result.ok_or(KernelError::OutputContractMismatch)
}

fn contextual_series(
    series: &SeriesHandle,
    operation: &mut SeriesTransform,
    inv: &KernelInvocation<'_>,
) -> Result<SeriesHandle, KernelError> {
    let window = match operation {
        SeriesTransform::Difference { window, .. }
        | SeriesTransform::PercentChange { window, .. }
        | SeriesTransform::Shift { window, .. }
        | SeriesTransform::Rolling { window, .. }
        | SeriesTransform::Cumulative { window, .. }
        | SeriesTransform::Rank { window, .. }
        | SeriesTransform::FillDirection { window, .. } => window,
        _ => return Ok(series.clone()),
    };
    let Some(context) = &window.context else {
        return Ok(series.clone());
    };
    if context.shares_row_domain(series.relation()) {
        return Ok(series.clone());
    }
    let (frame, series) =
        super::series::attach(context, &RuntimeValue::Series(series.clone()), inv)?;
    window.context = Some(frame);
    Ok(series)
}

pub(super) fn execute_series(
    kind: SeriesKernel,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    inv.check_control()?;
    use SeriesKernel::*;
    if matches!(kind, Range) {
        if integer(inv, "step")? == 0 {
            return Err(KernelError::InvalidParameter);
        }
        let relation = Arc::clone(inv.relations)
            .integer_range(
                integer(inv, "start")?,
                integer(inv, "end")?,
                integer(inv, "step")?,
                &inv.relation_control(),
            )
            .map_err(kernel_error)?;
        return Ok(vec![RuntimeValue::Series(
            relation.select_series("value").map_err(kernel_error)?,
        )]);
    }
    let input = inv
        .inputs
        .get(if matches!(kind, PanelDifference) {
            1
        } else {
            0
        })
        .ok_or(KernelError::InputLayoutMismatch)?;
    let series = series_input(input, inv)?;
    let reduction = match kind {
        Length => Some(SeriesReduction::Length),
        Count => Some(SeriesReduction::Count),
        Sum => Some(SeriesReduction::Sum),
        Mean => Some(SeriesReduction::Mean),
        _ => None,
    };
    if let Some(operation) = reduction {
        let result = scalar_results(
            &series
                .relation()
                .reduce_series(&series, operation)
                .map_err(kernel_error)?,
            inv,
        )?;
        return Ok(result.into_iter().map(RuntimeValue::Scalar).collect());
    }
    let mut operation = match kind {
        Standardize => {
            let stats = scalar_results(
                &series
                    .relation()
                    .reduce_series(&series, SeriesReduction::StandardizationStatistics)
                    .map_err(kernel_error)?,
                inv,
            )?;
            let [mean, sd] = stats.as_slice() else {
                return Err(KernelError::OutputContractMismatch);
            };
            let scale = numeric_input(Some(&RuntimeValue::Scalar(sd.clone())))?;
            if scale <= 0.0 {
                return Err(KernelError::InvalidNumericInput);
            }
            let result = series
                .relation()
                .transform_series(&series, &SeriesTransform::Standardize)
                .map_err(kernel_error)?;
            return Ok(vec![
                RuntimeValue::Series(result),
                RuntimeValue::Scalar(mean.clone()),
                RuntimeValue::Scalar(sd.clone()),
            ]);
        }
        InverseStandardize => SeriesTransform::InverseStandardize {
            mean: numeric_input(inv.inputs.get(1))?,
            standard_deviation: numeric_input(inv.inputs.get(2))?,
        },
        Dummy => SeriesTransform::DummyInformation {
            base_level: text(inv, "base_level")?.into(),
        },
        Difference => SeriesTransform::Difference {
            order: positive(inv, "order")?,
            window: SeriesWindow::default(),
        },
        PercentChange => SeriesTransform::PercentChange {
            periods: positive(inv, "order")?,
            window: SeriesWindow::default(),
        },
        RollingMean => {
            let size = positive(inv, "window")?;
            let minimum = integer(inv, "min_periods")?;
            SeriesTransform::Rolling {
                operation: window_operation(inv)?,
                size,
                min_periods: if minimum == 0 {
                    size
                } else {
                    usize::try_from(minimum).map_err(|_| KernelError::InvalidParameter)?
                },
                window: window(inv)?,
            }
        }
        Lag => SeriesTransform::Shift {
            periods: positive(inv, "window")?,
            lead: text(inv, "direction")? == "lead",
            window: window(inv)?,
        },
        PanelDifference => {
            let Some(RuntimeValue::Relation(frame)) = inv.inputs.first() else {
                return Err(KernelError::InvalidParameter);
            };
            SeriesTransform::Difference {
                order: positive(inv, "order")?,
                window: SeriesWindow {
                    context: Some(frame.clone()),
                    partition_by: vec![text(inv, "entity_column")?.into()],
                    order_by: vec![SortColumn {
                        column: text(inv, "time_column")?.into(),
                        ascending: true,
                        nulls_first: false,
                    }],
                    require_unique_keys: true,
                },
            }
        }
        _ => return Err(KernelError::InvalidParameter),
    };
    let series = contextual_series(&series, &mut operation, inv)?;
    Ok(vec![RuntimeValue::Series(
        series
            .relation()
            .transform_series(&series, &operation)
            .map_err(kernel_error)?,
    )])
}

fn execute(
    operation: Operation,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    inv.check_control()?;
    use Operation::*;
    if matches!(
        operation,
        Sort | Deduplicate | SetColumn | Mask | Unpivot | Pivot | Resample
    ) {
        let Some(RuntimeValue::Relation(source)) = inv.inputs.first() else {
            return Err(KernelError::InvalidParameter);
        };
        let result = match operation {
            Sort => {
                let descending = strings(inv, "descending_columns")?;
                let columns = strings(inv, "columns")?;
                let nulls_first = boolean(inv, "nulls_first")?;
                if descending.iter().any(|name| !columns.contains(name)) {
                    return Err(KernelError::InvalidParameter);
                }
                source.sort_rows(
                    &columns
                        .into_iter()
                        .map(|column| SortColumn {
                            ascending: !descending.contains(&column),
                            column,
                            nulls_first,
                        })
                        .collect::<Vec<_>>(),
                )
            }
            Deduplicate => source.deduplicate(
                &strings(inv, "keys")?,
                match text(inv, "keep")? {
                    "first" => DuplicateKeep::First,
                    "last" => DuplicateKeep::Last,
                    "none" => DuplicateKeep::None,
                    _ => return Err(KernelError::InvalidParameter),
                },
            ),
            SetColumn => {
                let operand = inv.inputs.get(1).ok_or(KernelError::InputLayoutMismatch)?;
                if let RuntimeValue::Series(series) = operand.unannotated()
                    && source.shares_row_domain(series.relation())
                {
                    source.set_column(text(inv, "name")?, series)
                } else {
                    let (frame, series) = super::series::attach(source, &inv.inputs[1], inv)?;
                    let mut names = frame
                        .schema()
                        .fields()
                        .iter()
                        .take(frame.schema().fields().len() - 1)
                        .map(|field| field.name().clone().into_boxed_str())
                        .collect::<Vec<_>>();
                    let name = text(inv, "name")?;
                    if !names.iter().any(|n| n.as_ref() == name) {
                        names.push(name.into());
                    }
                    frame
                        .set_column(name, &series)
                        .and_then(|frame| frame.project(&names))
                }
            }
            Mask => {
                let operand = inv.inputs.get(1).ok_or(KernelError::InputLayoutMismatch)?;
                if let RuntimeValue::Series(mask) = operand.unannotated()
                    && source.shares_row_domain(mask.relation())
                {
                    source.filter_mask(mask, boolean(inv, "drop_matches")?)
                } else {
                    let (frame, mask) = super::series::attach(source, &inv.inputs[1], inv)?;
                    let names = frame
                        .schema()
                        .fields()
                        .iter()
                        .take(frame.schema().fields().len() - 1)
                        .map(|field| field.name().clone().into_boxed_str())
                        .collect::<Vec<_>>();
                    frame
                        .filter_mask(&mask, boolean(inv, "drop_matches")?)
                        .and_then(|frame| frame.project(&names))
                }
            }
            Unpivot => source.unpivot(&UnpivotSpec {
                keys: strings(inv, "keys")?,
                columns: strings(inv, "columns")?,
                variable_name: text(inv, "variable_name")?.into(),
                value_name: text(inv, "value_name")?.into(),
                include_null: boolean(inv, "include_null")?,
            }),
            Pivot => source.pivot(&PivotSpec {
                keys: strings(inv, "keys")?,
                category: text(inv, "category_column")?.into(),
                value: text(inv, "value_column")?.into(),
                levels: levels(inv)?,
                aggregate: aggregate(inv)?,
            }),
            Resample => source.resample(&ResampleSpec {
                time: text(inv, "time_column")?.into(),
                unit: text(inv, "unit")?.into(),
                keys: strings(inv, "keys")?,
                columns: strings(inv, "columns")?,
                aggregate: aggregate(inv)?,
            }),
            _ => unreachable!(),
        }
        .map_err(kernel_error)?;
        return Ok(vec![RuntimeValue::Relation(result)]);
    }
    let inputs = inv
        .inputs
        .iter()
        .zip(inv.input_keys)
        .filter(|(_, key)| **key != "context")
        .map(|(value, _)| value)
        .collect::<Vec<_>>();
    let (relation, operands) = operands(&inputs, inv)?;
    if matches!(operation, Concatenate) {
        return Ok(vec![RuntimeValue::Series(
            relation
                .concatenate_text(&operands, text(inv, "separator")?)
                .map_err(kernel_error)?,
        )]);
    }
    let Some(SeriesOperand::Series(series)) = operands.first() else {
        return Err(KernelError::InvalidParameter);
    };
    let mut operation = match operation {
        Choose => {
            let [SeriesOperand::Series(condition), yes, no] = operands.as_slice() else {
                return Err(KernelError::InputLayoutMismatch);
            };
            let prototype =
                if let (SeriesOperand::Scalar(yes), SeriesOperand::Scalar(no)) = (yes, no) {
                    let Some(ValueType::DataSeries(element)) =
                        inv.outputs.first().map(|o| &o.data_type)
                    else {
                        return Err(KernelError::OutputContractMismatch);
                    };
                    let ValueType::Scalar(kind) = element.as_ref() else {
                        return Err(KernelError::OutputContractMismatch);
                    };
                    let metadata = inputs[1]
                        .metadata()
                        .or_else(|| inputs[2].metadata())
                        .cloned()
                        .unwrap_or(yss_data_contract::ConversionMetadata {
                            semantic: yss_data_contract::ColumnSemantic::new(*kind),
                            temporal: None,
                            dummy_base_level: None,
                        });
                    Some(
                        yss_database_arrow::materialized_column(
                            "result",
                            &[yes.clone(), no.clone()],
                            Some(&metadata),
                        )
                        .map_err(|_| KernelError::InvalidParameter)?
                        .0,
                    )
                } else {
                    None
                };
            return Ok(vec![RuntimeValue::Series(
                relation
                    .choose_series(condition, yes, no, prototype.as_ref())
                    .map_err(kernel_error)?,
            )]);
        }
        Encode => {
            let drop = boolean(inv, "drop_reference")?;
            return Ok(vec![RuntimeValue::Relation(
                relation
                    .encode_series(
                        series,
                        &levels(inv)?,
                        if drop {
                            Some(text(inv, "reference")?)
                        } else {
                            None
                        },
                    )
                    .map_err(kernel_error)?,
            )]);
        }
        IsNull(invert) => SeriesTransform::IsNull { invert },
        Impute => SeriesTransform::Impute {
            method: match text(inv, "imputation_method")? {
                "mean" => ImputationMethod::Mean,
                "median" => ImputationMethod::Median,
                "mode" => ImputationMethod::Mode,
                "constant" => {
                    ImputationMethod::Constant(numeric_input(inv.parameter("fill_value"))?)
                }
                _ => return Err(KernelError::InvalidParameter),
            },
        },
        Fill => SeriesTransform::Fill {
            replacement: operands
                .get(1)
                .ok_or(KernelError::InputLayoutMismatch)?
                .clone(),
        },
        Map => SeriesTransform::Map {
            from: values(inv, "from_values")?,
            to: values(inv, "to_values")?,
            keep_unmatched: boolean(inv, "keep_unmatched")?,
        },
        Trim => SeriesTransform::Trim,
        Lower => SeriesTransform::Lower,
        Upper => SeriesTransform::Upper,
        Replace => SeriesTransform::Replace {
            from: text(inv, "from")?.into(),
            to: text(inv, "to")?.into(),
        },
        Substring => SeriesTransform::Substring {
            start: integer(inv, "start")?,
            length: integer(inv, "length")?,
        },
        Split => SeriesTransform::SplitPart {
            separator: text(inv, "separator")?.into(),
            part: integer(inv, "part")?,
        },
        DatePart => SeriesTransform::DatePart {
            part: text(inv, "part")?.into(),
        },
        DateTruncate => SeriesTransform::DateTruncate {
            unit: text(inv, "unit")?.into(),
        },
        DateAdd => SeriesTransform::DateAdd {
            unit: text(inv, "unit")?.into(),
            amount: integer(inv, "amount")?,
        },
        DateDifference => SeriesTransform::DateDifference {
            unit: text(inv, "unit")?.into(),
            other: operands
                .get(1)
                .ok_or(KernelError::InputLayoutMismatch)?
                .clone(),
        },
        Clip => SeriesTransform::Clip {
            lower: numeric_input(inv.parameter("lower"))?,
            upper: numeric_input(inv.parameter("upper"))?,
        },
        Bin => SeriesTransform::Bin {
            edges: values(inv, "edges")?
                .into_iter()
                .map(|v| numeric_input(Some(&RuntimeValue::Scalar(v))))
                .collect::<Result<_, _>>()?,
            labels: strings(inv, "labels")?,
        },
        Cumulative => SeriesTransform::Cumulative {
            operation: window_operation(inv)?,
            window: window(inv)?,
        },
        Rank => SeriesTransform::Rank {
            dense: boolean(inv, "dense")?,
            descending: boolean(inv, "descending")?,
            nulls_first: boolean(inv, "nulls_first")?,
            window: window(inv)?,
        },
        ForwardFill => SeriesTransform::FillDirection {
            forward: true,
            window: window(inv)?,
        },
        BackwardFill => SeriesTransform::FillDirection {
            forward: false,
            window: window(inv)?,
        },
        _ => unreachable!(),
    };
    let series = contextual_series(series, &mut operation, inv)?;
    Ok(vec![RuntimeValue::Series(
        series
            .relation()
            .transform_series(&series, &operation)
            .map_err(kernel_error)?,
    )])
}
