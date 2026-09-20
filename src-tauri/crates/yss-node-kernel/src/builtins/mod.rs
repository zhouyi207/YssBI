use yss_data_contract::TabularScalar;
mod boolean;
mod comparison;
mod conversion;
mod numeric;
mod relational;
mod series;
mod statistics;

use crate::{KernelError, KernelInvocation, RuntimeValue};
use yss_relational_contract::NumericOperation;

#[derive(Clone, Copy)]
enum BuiltinKernel {
    Statistical(statistics::StatisticalKernel),
    Relational(relational::RelationalKernel),
    Series(series::SeriesKernel),
    Decompose,
    Constant,
    FixedNumber(f64),
    Numeric(NumericOperation),
    Boolean(yss_relational_contract::BooleanOperation),
    Comparison(yss_relational_contract::ComparisonOperation),
    WholeEqual,
    Convert,
    Observe,
}

pub(crate) fn register_builtin_kernels(builder: &mut crate::KernelRegistryBuilder) {
    use BuiltinKernel::*;
    use NumericOperation::{
        Add, Divide, Ln, Log2, Log10, Logarithm, Multiply, Power, Sqrt, Square, Subtract,
    };
    use statistics::StatisticalKernel::{LinearFit, LinearPredict, LinearRegressionSummary};
    let entries: &[(
        &str,
        BuiltinKernel,
        &[&str],
        std::ops::RangeInclusive<usize>,
    )] = &[
        (
            "yssbi.dataframe.series.int_range",
            Series(series::SeriesKernel::Range),
            &["configuration"],
            1..=1,
        ),
        (
            "yssbi.dataframe.series.length",
            Series(series::SeriesKernel::Length),
            &[],
            1..=1,
        ),
        (
            "yssbi.dataframe.series.count",
            Series(series::SeriesKernel::Count),
            &[],
            1..=1,
        ),
        (
            "yssbi.dataframe.series.sum",
            Series(series::SeriesKernel::Sum),
            &[],
            1..=1,
        ),
        (
            "yssbi.dataframe.series.mean",
            Series(series::SeriesKernel::Mean),
            &[],
            1..=1,
        ),
        (
            "yssbi.dataframe.series.standardize",
            Series(series::SeriesKernel::Standardize),
            &[],
            3..=3,
        ),
        (
            "yssbi.dataframe.series.inverse_standardize",
            Series(series::SeriesKernel::InverseStandardize),
            &[],
            1..=1,
        ),
        (
            "yssbi.dataframe.series.annotate_dummy",
            Series(series::SeriesKernel::Dummy),
            &["base_level"],
            1..=1,
        ),
        (
            "yssbi.dataframe.timeseries.difference",
            Series(series::SeriesKernel::Difference),
            &["order"],
            1..=1,
        ),
        (
            "yssbi.dataframe.timeseries.percent_change",
            Series(series::SeriesKernel::PercentChange),
            &["order"],
            1..=1,
        ),
        (
            "yssbi.dataframe.timeseries.rolling_mean",
            Series(series::SeriesKernel::RollingMean),
            &["window"],
            1..=1,
        ),
        (
            "yssbi.dataframe.timeseries.lag",
            Series(series::SeriesKernel::Lag),
            &["window"],
            1..=1,
        ),
        (
            "yssbi.dataframe.panel.difference",
            Series(series::SeriesKernel::PanelDifference),
            &["order", "entity_column", "time_column"],
            1..=1,
        ),
        (
            "yssbi.statistics.linear.fit",
            Statistical(LinearFit),
            &["configuration"],
            3..=3,
        ),
        (
            "yssbi.statistics.linear.summary",
            Statistical(LinearRegressionSummary),
            &[],
            2..=2,
        ),
        (
            "yssbi.statistics.linear.predict",
            Statistical(LinearPredict),
            &[],
            1..=1,
        ),
        (
            "yssbi.dataframe.source.get",
            Relational(relational::RelationalKernel::Source),
            &["dataframe"],
            1..=1,
        ),
        (
            "yssbi.dataframe.project",
            Relational(relational::RelationalKernel::Project),
            &["columns"],
            1..=1,
        ),
        (
            "yssbi.dataframe.filter.rows",
            Relational(relational::RelationalKernel::Filter),
            &["predicate"],
            1..=1,
        ),
        (
            "yssbi.dataframe.drop.columns",
            Relational(relational::RelationalKernel::DropColumns),
            &["columns"],
            1..=1,
        ),
        (
            "yssbi.dataframe.drop.rows",
            Relational(relational::RelationalKernel::DropRows),
            &["predicate"],
            1..=1,
        ),
        (
            "yssbi.dataframe.dropna.rows",
            Relational(relational::RelationalKernel::DropNaRows),
            &["subset", "how"],
            1..=1,
        ),
        (
            "yssbi.dataframe.dropna.columns",
            Relational(relational::RelationalKernel::DropNaColumns),
            &["subset", "how"],
            1..=1,
        ),
        (
            "yssbi.dataframe.series.select",
            Relational(relational::RelationalKernel::Series),
            &["column"],
            1..=1,
        ),
        ("yssbi.dataframe.decompose", Decompose, &[], 0..=usize::MAX),
        (
            "yssbi.dataframe.combine",
            Relational(relational::RelationalKernel::Assemble),
            &[],
            1..=1,
        ),
        (
            "yssbi.dataframe.concat.rows",
            Relational(relational::RelationalKernel::ConcatRows),
            &["column_match"],
            1..=1,
        ),
        (
            "yssbi.dataframe.concat.columns",
            Relational(relational::RelationalKernel::ConcatColumns),
            &[],
            1..=1,
        ),
        (
            "yssbi.dataframe.join",
            Relational(relational::RelationalKernel::Join),
            &["left_keys", "right_keys", "join_type", "right_suffix"],
            1..=1,
        ),
        (
            "yssbi.dataframe.limit",
            Relational(relational::RelationalKernel::Limit),
            &["rows"],
            1..=1,
        ),
        (
            "yssbi.dataframe.rename",
            Relational(relational::RelationalKernel::Rename),
            &["from", "to"],
            1..=1,
        ),
        ("yssbi.constant.get", Constant, &["value"], 1..=1),
        (
            "yssbi.constant.pi",
            FixedNumber(std::f64::consts::PI),
            &[],
            1..=1,
        ),
        (
            "yssbi.constant.e",
            FixedNumber(std::f64::consts::E),
            &[],
            1..=1,
        ),
        ("yssbi.numeric.add", Numeric(Add), &[], 1..=1),
        ("yssbi.numeric.subtract", Numeric(Subtract), &[], 1..=1),
        ("yssbi.numeric.multiply", Numeric(Multiply), &[], 1..=1),
        ("yssbi.numeric.divide", Numeric(Divide), &[], 1..=1),
        ("yssbi.numeric.power", Numeric(Power), &[], 1..=1),
        ("yssbi.numeric.log", Numeric(Logarithm), &[], 1..=1),
        ("yssbi.numeric.ln", Numeric(Ln), &[], 1..=1),
        ("yssbi.numeric.log2", Numeric(Log2), &[], 1..=1),
        ("yssbi.numeric.log10", Numeric(Log10), &[], 1..=1),
        ("yssbi.numeric.square", Numeric(Square), &[], 1..=1),
        ("yssbi.numeric.sqrt", Numeric(Sqrt), &[], 1..=1),
        ("yssbi.compare.whole_equal", WholeEqual, &[], 1..=1),
        (
            "yssbi.logic.and",
            Boolean(yss_relational_contract::BooleanOperation::And),
            &[],
            1..=1,
        ),
        (
            "yssbi.logic.or",
            Boolean(yss_relational_contract::BooleanOperation::Or),
            &[],
            1..=1,
        ),
        (
            "yssbi.logic.not",
            Boolean(yss_relational_contract::BooleanOperation::Not),
            &[],
            1..=1,
        ),
        (
            "yssbi.compare.equal",
            Comparison(yss_relational_contract::ComparisonOperation::Equal),
            &[],
            1..=1,
        ),
        (
            "yssbi.compare.not_equal",
            Comparison(yss_relational_contract::ComparisonOperation::NotEqual),
            &[],
            1..=1,
        ),
        (
            "yssbi.compare.less",
            Comparison(yss_relational_contract::ComparisonOperation::Less),
            &[],
            1..=1,
        ),
        (
            "yssbi.compare.less_equal",
            Comparison(yss_relational_contract::ComparisonOperation::LessEqual),
            &[],
            1..=1,
        ),
        (
            "yssbi.compare.greater",
            Comparison(yss_relational_contract::ComparisonOperation::Greater),
            &[],
            1..=1,
        ),
        (
            "yssbi.compare.greater_equal",
            Comparison(yss_relational_contract::ComparisonOperation::GreaterEqual),
            &[],
            1..=1,
        ),
        (
            "yssbi.value.convert",
            Convert,
            &[
                "target_type",
                "numeric_mode",
                "semantic_domain",
                "datetime_kind",
                "datetime_precision",
                "datetime_format",
            ],
            1..=1,
        ),
        ("yssbi.debug.view", Observe, &[], 0..=0),
    ];
    for (id, kind, parameters, outputs) in entries {
        let kind = *kind;
        let contract = crate::KernelContract::new(
            input_contract(kind),
            parameters.iter().map(|field| {
                crate::KernelParameterKey::new((*field).into())
                    .expect("built-in parameter identity")
            }),
            outputs.clone(),
        )
        .expect("built-in kernel contract");
        builder
            .register(
                crate::KernelId::new((*id).into()).expect("built-in kernel identity"),
                std::num::NonZeroU32::new(match kind {
                    Comparison(_) => 5,
                    Boolean(_) => 3,
                    Statistical(LinearFit | LinearRegressionSummary | LinearPredict) => 4,
                    Convert => 6,
                    Numeric(_) | Relational(relational::RelationalKernel::Filter) => 3,
                    _ => 2,
                })
                .expect("built-in implementation revision"),
                contract,
                move |invocation| execute_kernel(kind, invocation),
            )
            .expect("built-in kernels have distinct identities");
    }
}

fn input_contract(kind: BuiltinKernel) -> Vec<crate::KernelInputSpec> {
    use crate::KernelInputSpec as Input;
    use BuiltinKernel::*;
    use relational::RelationalKernel as Table;
    use statistics::StatisticalKernel as Stats;
    match kind {
        Series(series::SeriesKernel::Range) => vec![],
        Series(series::SeriesKernel::InverseStandardize) => vec![
            Input::fixed("standardized"),
            Input::fixed("mean"),
            Input::fixed("standard_deviation"),
        ],
        Series(series::SeriesKernel::Dummy) => vec![Input::fixed("source")],
        Series(series::SeriesKernel::PanelDifference) => {
            vec![Input::fixed("aligned"), Input::fixed("series")]
        }
        Series(_) => vec![Input::fixed("series")],
        Statistical(Stats::LinearFit) => vec![
            Input::fixed("response"),
            Input::repeated("predictors", 1..=usize::MAX),
            Input::repeated("weights", 0..=1),
            Input::repeated("sigma", 0..=usize::MAX),
        ],
        Statistical(Stats::LinearPredict) => vec![
            Input::fixed("model"),
            Input::repeated("predictors", 1..=usize::MAX),
        ],
        Statistical(Stats::LinearRegressionSummary) => vec![Input::fixed("model")],
        Relational(Table::Source) | Constant | FixedNumber(_) => vec![],
        Relational(Table::Assemble) => vec![Input::repeated("series", 1..=usize::MAX)],
        Relational(Table::ConcatRows | Table::ConcatColumns) => {
            vec![Input::repeated("frames", 2..=usize::MAX)]
        }
        Relational(Table::Join) => vec![Input::fixed("left"), Input::fixed("right")],
        Relational(Table::Series) | Decompose => vec![Input::fixed("dataframe")],
        Relational(_) => vec![Input::fixed("source")],
        Numeric(NumericOperation::Add) => vec![Input::repeated("operands", 2..=usize::MAX)],
        Numeric(op) if op.is_unary() => vec![Input::fixed("input")],
        Boolean(yss_relational_contract::BooleanOperation::Not) | Convert => {
            vec![Input::fixed("input")]
        }
        Numeric(_) | Boolean(_) | Comparison(_) | WholeEqual => {
            vec![Input::fixed("left"), Input::fixed("right")]
        }
        Observe => vec![Input::fixed("data")],
    }
}

fn execute_kernel(
    kind: BuiltinKernel,
    invocation: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let inputs = invocation.inputs;
    let value = match kind {
        BuiltinKernel::Series(kind) => return series::execute(kind, invocation),
        BuiltinKernel::Statistical(kind) => {
            return statistics::execute(kind, invocation);
        }
        BuiltinKernel::Relational(kind) => relational::execute(kind, invocation),
        BuiltinKernel::Decompose => return relational::decompose(invocation),
        BuiltinKernel::Constant => relational::constant(invocation),
        BuiltinKernel::FixedNumber(value) => {
            RuntimeValue::float64(value).map_err(|_| KernelError::NonFiniteResult)
        }
        BuiltinKernel::Numeric(operation) => numeric::execute(operation, invocation),
        BuiltinKernel::Boolean(operation) => boolean::execute(operation, invocation),
        BuiltinKernel::Comparison(operation) => comparison::execute(operation, invocation),
        BuiltinKernel::WholeEqual => {
            let [left, right] = inputs else {
                return Err(KernelError::Failed);
            };
            fn materialized(value: &RuntimeValue) -> bool {
                match value.unannotated() {
                    RuntimeValue::List(values) => values.iter().all(materialized),
                    RuntimeValue::Record(values) => values.values().all(materialized),
                    value => value.tabular_scalar().is_ok(),
                }
            }
            if !materialized(left) || !materialized(right) {
                return Err(KernelError::Failed);
            }
            Ok(RuntimeValue::Scalar(TabularScalar::Bool(
                left.semantic_eq(right),
            )))
        }
        BuiltinKernel::Convert => conversion::execute(invocation),
        BuiltinKernel::Observe => return Ok(Vec::new()),
    }?;
    Ok(vec![value])
}

pub(crate) fn numeric_input(value: Option<&RuntimeValue>) -> Result<f64, KernelError> {
    match value {
        Some(RuntimeValue::Scalar(TabularScalar::Integer(value)))
            if (*value as f64) as i128 == i128::from(*value) =>
        {
            Ok(*value as f64)
        }
        Some(RuntimeValue::Scalar(TabularScalar::Unsigned(value)))
            if (*value as f64) as u128 == u128::from(*value) =>
        {
            Ok(*value as f64)
        }
        Some(RuntimeValue::Scalar(TabularScalar::Float64(value))) => Ok(value.as_f64()),
        _ => Err(KernelError::InvalidNumericInput),
    }
}
