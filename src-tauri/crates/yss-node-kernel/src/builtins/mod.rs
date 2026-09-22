use yss_data_contract::TabularScalar;
mod boolean;
mod comparison;
mod conversion;
mod distribution;
mod numeric;
mod relational;
mod series;
mod statistics;

use crate::{KernelError, KernelInvocation, RuntimeValue};
use yss_relational_contract::NumericOperation;

#[derive(Clone, Copy)]
enum BuiltinKernel {
    Distribution(distribution::DistributionKernel),
    Statistical(statistics::LinearKernel),
    Relational(relational::RelationalKernel),
    Series(series::SeriesKernel),
    Decompose,
    Constant,
    FixedNumber(f64),
    Numeric(NumericOperation),
    Boolean(yss_relational_contract::BooleanOperation),
    Comparison(yss_relational_contract::ComparisonOperation),
    Convert,
    Observe,
}

pub(crate) fn register_builtin_kernels(builder: &mut crate::KernelRegistryBuilder) {
    use BuiltinKernel::*;
    use NumericOperation::{
        Add, Divide, Ln, Log2, Log10, Logarithm, Multiply, Power, Sqrt, Square, Subtract,
    };
    use statistics::LinearKernel::{Fit, Predict, Summary};
    let entries: &[(
        &str,
        BuiltinKernel,
        &[&str],
        std::ops::RangeInclusive<usize>,
    )] = &[
        (
            "yssbi.distribution.normal.sample",
            Distribution(distribution::DistributionKernel::Normal),
            &["mean", "standard_deviation", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.uniform.sample",
            Distribution(distribution::DistributionKernel::Uniform),
            &["lower_bound", "upper_bound", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.exponential.sample",
            Distribution(distribution::DistributionKernel::Exponential),
            &["rate", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.gamma.sample",
            Distribution(distribution::DistributionKernel::Gamma),
            &["shape", "rate", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.beta.sample",
            Distribution(distribution::DistributionKernel::Beta),
            &["alpha", "beta", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.students_t.sample",
            Distribution(distribution::DistributionKernel::StudentsT),
            &["degrees_of_freedom", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.cauchy.sample",
            Distribution(distribution::DistributionKernel::Cauchy),
            &["location", "scale", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.chi_squared.sample",
            Distribution(distribution::DistributionKernel::ChiSquared),
            &["degrees_of_freedom", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.log_normal.sample",
            Distribution(distribution::DistributionKernel::LogNormal),
            &["mu", "sigma", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.weibull.sample",
            Distribution(distribution::DistributionKernel::Weibull),
            &["shape", "scale", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.laplace.sample",
            Distribution(distribution::DistributionKernel::Laplace),
            &["location", "scale", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.pareto.sample",
            Distribution(distribution::DistributionKernel::Pareto),
            &["shape", "scale", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.inverse_gamma.sample",
            Distribution(distribution::DistributionKernel::InverseGamma),
            &["shape", "scale", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.triangular.sample",
            Distribution(distribution::DistributionKernel::Triangular),
            &["minimum", "maximum", "mode", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.fisher_snedecor.sample",
            Distribution(distribution::DistributionKernel::FisherSnedecor),
            &[
                "numerator_degrees_of_freedom",
                "denominator_degrees_of_freedom",
                "sample_count",
            ],
            1..=1,
        ),
        (
            "yssbi.distribution.erlang.sample",
            Distribution(distribution::DistributionKernel::Erlang),
            &["shape", "rate", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.bernoulli.sample",
            Distribution(distribution::DistributionKernel::Bernoulli),
            &["probability", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.binomial.sample",
            Distribution(distribution::DistributionKernel::Binomial),
            &["trial_count", "probability", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.poisson.sample",
            Distribution(distribution::DistributionKernel::Poisson),
            &["rate", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.geometric.sample",
            Distribution(distribution::DistributionKernel::Geometric),
            &["probability", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.negative_binomial.sample",
            Distribution(distribution::DistributionKernel::NegativeBinomial),
            &["success_count", "probability", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.discrete_uniform.sample",
            Distribution(distribution::DistributionKernel::DiscreteUniform),
            &["lower_bound", "upper_bound", "sample_count"],
            1..=1,
        ),
        (
            "yssbi.distribution.hypergeometric.sample",
            Distribution(distribution::DistributionKernel::Hypergeometric),
            &[
                "population_size",
                "success_population",
                "draw_count",
                "sample_count",
            ],
            1..=1,
        ),
        (
            "yssbi.dataframe.series.int_range",
            Series(series::SeriesKernel::Range),
            &["start", "end", "step"],
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
            Statistical(Fit),
            &[
                "method",
                "constant",
                "covariance",
                "kernel",
                "bandwidth",
                "lag",
                "scale",
            ],
            3..=3,
        ),
        (
            "yssbi.statistics.linear.summary",
            Statistical(Summary),
            &[
                "equation",
                "model_summary",
                "anova",
                "coefficient_table",
                "coefficient_chart",
                "diagnostics",
                "residual_plot",
                "observations",
                "acf_pacf",
                "acf_max_lag",
                "serial_tests",
                "serial_lags",
                "bg_nomiss0",
                "hypothesis_test",
                "hypothesis",
            ],
            2..=2,
        ),
        (
            "yssbi.statistics.linear.predict",
            Statistical(Predict),
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
            &["mode", "absolute_tolerance", "relative_tolerance"],
            1..=1,
        ),
        (
            "yssbi.compare.not_equal",
            Comparison(yss_relational_contract::ComparisonOperation::NotEqual),
            &["mode", "absolute_tolerance", "relative_tolerance"],
            1..=1,
        ),
        (
            "yssbi.compare.less",
            Comparison(yss_relational_contract::ComparisonOperation::Less),
            &["mode", "absolute_tolerance", "relative_tolerance"],
            1..=1,
        ),
        (
            "yssbi.compare.less_equal",
            Comparison(yss_relational_contract::ComparisonOperation::LessEqual),
            &["mode", "absolute_tolerance", "relative_tolerance"],
            1..=1,
        ),
        (
            "yssbi.compare.greater",
            Comparison(yss_relational_contract::ComparisonOperation::Greater),
            &["mode", "absolute_tolerance", "relative_tolerance"],
            1..=1,
        ),
        (
            "yssbi.compare.greater_equal",
            Comparison(yss_relational_contract::ComparisonOperation::GreaterEqual),
            &["mode", "absolute_tolerance", "relative_tolerance"],
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
        let optional: &[&str] = match kind {
            Comparison(_) => &["absolute_tolerance", "relative_tolerance"],
            Statistical(Fit) => &["kernel", "bandwidth", "lag", "scale"],
            Statistical(Summary) => &["acf_max_lag", "serial_lags", "bg_nomiss0", "hypothesis"],
            _ => &[],
        };
        let contract =
            contract
                .with_optional_parameters(optional.iter().map(|key| {
                    crate::KernelParameterKey::new((*key).into()).expect("parameter key")
                }))
                .expect("declared optional parameters");
        builder
            .register(
                crate::KernelId::new((*id).into()).expect("built-in kernel identity"),
                std::num::NonZeroU32::new(match kind {
                    Comparison(_) => 8,
                    Distribution(_) | Series(series::SeriesKernel::Range) => 3,
                    Statistical(Fit) => 6,
                    Series(
                        series::SeriesKernel::Standardize
                        | series::SeriesKernel::InverseStandardize,
                    ) => 3,
                    Boolean(_) => 3,
                    Statistical(Summary) => 5,
                    Statistical(Predict) => 4,
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
    use statistics::LinearKernel as Stats;
    match kind {
        Distribution(_) => vec![],
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
        Statistical(Stats::Fit) => vec![
            Input::fixed("response"),
            Input::repeated("predictors", 1..=usize::MAX),
            Input::repeated("weights", 0..=1),
            Input::repeated("sigma", 0..=usize::MAX),
        ],
        Statistical(Stats::Predict) => vec![
            Input::fixed("model"),
            Input::repeated("predictors", 1..=usize::MAX),
        ],
        Statistical(Stats::Summary) => vec![Input::fixed("model")],
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
        Numeric(_) | Boolean(_) | Comparison(_) => {
            vec![Input::fixed("left"), Input::fixed("right")]
        }
        Observe => vec![Input::fixed("data")],
    }
}

fn execute_kernel(
    kind: BuiltinKernel,
    invocation: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let value = match kind {
        BuiltinKernel::Distribution(kind) => distribution::execute(kind, invocation),
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
