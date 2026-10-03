//! Controlled materialization and SCI plot adapters; no renderer or graph identity.
use super::statistics::{common::*, install};
use super::{numeric_input, series};
use crate::{
    KernelError, KernelInputSpec as Input, KernelInvocation, KernelRegistryBuilder, RuntimeValue,
};
use yss_data_contract::TabularScalar;
use yss_sci_contract::execution::{ScientificComputationError, ScientificExecutionControl};
use yss_sci_contract::visualization::*;
use yss_sci_runtime::visualization as sci;
mod overview;

#[derive(Clone, Copy)]
enum Method {
    Scatter,
    Line,
    Ecdf,
    Kde,
    Histogram,
    Correlation,
    Correlogram,
    Box,
    WordCloud,
    ErrorBars,
    Probability,
    Roc,
    Quadrant,
    Pareto,
    Combination,
    Bubble,
    Violin,
    Heatmap,
    Coefficients,
}

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    overview::register(builder);
    use Method::*;
    for (name, method, inputs, parameters) in [
        (
            "scatter",
            Scatter,
            vec![Input::fixed("x"), Input::fixed("y")],
            vec![],
        ),
        (
            "line",
            Line,
            vec![Input::fixed("x"), Input::fixed("y")],
            vec![],
        ),
        ("ecdf", Ecdf, vec![Input::fixed("values")], vec![]),
        (
            "kde",
            Kde,
            vec![Input::fixed("values")],
            vec!["grid_points"],
        ),
        (
            "histogram",
            Histogram,
            vec![Input::fixed("values")],
            vec!["bins"],
        ),
        (
            "correlation",
            Correlation,
            vec![Input::repeated("series", 2..=MAX_PLOT_GROUPS)],
            vec![],
        ),
        (
            "correlogram",
            Correlogram,
            vec![Input::fixed("values")],
            vec!["maximum_lag"],
        ),
        (
            "boxplot",
            Box,
            vec![Input::repeated("series", 1..=MAX_PLOT_GROUPS)],
            vec![],
        ),
        (
            "wordcloud",
            WordCloud,
            vec![Input::fixed("words")],
            vec!["max_words"],
        ),
        (
            "errorbar",
            ErrorBars,
            vec![
                Input::fixed("x"),
                Input::fixed("y"),
                Input::fixed("lower"),
                Input::fixed("upper"),
            ],
            vec![],
        ),
        (
            "pp_qq",
            Probability,
            vec![Input::fixed("values")],
            vec![
                "mode",
                "estimate_parameters",
                "reference_mean",
                "reference_standard_deviation",
            ],
        ),
        (
            "roc",
            Roc,
            vec![Input::fixed("labels"), Input::fixed("scores")],
            vec![],
        ),
        (
            "quadrant",
            Quadrant,
            vec![Input::fixed("x"), Input::fixed("y")],
            vec!["x_cut", "y_cut"],
        ),
        ("pareto", Pareto, vec![Input::fixed("categories")], vec![]),
        (
            "combination",
            Combination,
            vec![
                Input::fixed("categories"),
                Input::fixed("bars"),
                Input::fixed("line"),
            ],
            vec!["dual_axis"],
        ),
        (
            "bubble",
            Bubble,
            vec![Input::fixed("x"), Input::fixed("y"), Input::fixed("size")],
            vec![],
        ),
        (
            "violin",
            Violin,
            vec![Input::repeated("series", 1..=MAX_PLOT_GROUPS)],
            vec![],
        ),
        (
            "heatmap",
            Heatmap,
            vec![Input::repeated("series", 1..=MAX_PLOT_GROUPS)],
            vec![],
        ),
        (
            "coefficient",
            Coefficients,
            vec![Input::fixed("model")],
            vec!["confidence_level", "include_intercept"],
        ),
    ] {
        install(
            builder,
            &format!("yssbi.plot.{name}.view"),
            inputs,
            &parameters,
            1,
            move |inv| execute(method, inv),
        );
    }
}

fn encode<T: serde::Serialize>(
    result: Result<T, ScientificComputationError>,
    inv: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    value(result.map_err(computation_error)?, inv)
}

fn prepared(
    inv: &KernelInvocation<'_>,
    independent: bool,
) -> Result<Vec<series::Column>, KernelError> {
    inv.check_control()?;
    let all_handles = inv
        .inputs
        .iter()
        .all(|value| matches!(value.unannotated(), RuntimeValue::Series(_)));
    let any_handles = inv
        .inputs
        .iter()
        .any(|value| matches!(value.unannotated(), RuntimeValue::Series(_)));
    if !independent && any_handles && !all_handles {
        return Err(KernelError::UnalignedSeries);
    }
    let data = if all_handles && !independent {
        let handles = inv
            .inputs
            .iter()
            .map(|value| match value.unannotated() {
                RuntimeValue::Series(series) => series.clone(),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        series::load(&handles, inv)?
    } else {
        let mut columns = Vec::with_capacity(inv.inputs.len());
        let mut retained = 0usize;
        for input in inv.inputs {
            let column = series::column(input, inv)?;
            retained = retained
                .checked_add(
                    column
                        .values
                        .len()
                        .checked_mul(size_of::<TabularScalar>() * 4)
                        .ok_or(KernelError::BudgetExceeded)?,
                )
                .ok_or(KernelError::BudgetExceeded)?;
            for value in &column.values {
                if let TabularScalar::String(value) = value {
                    retained = retained
                        .checked_add(
                            value
                                .len()
                                .checked_mul(4)
                                .ok_or(KernelError::BudgetExceeded)?,
                        )
                        .ok_or(KernelError::BudgetExceeded)?;
                }
            }
            inv.control.check_bytes(Some(retained))?;
            columns.push(column);
        }
        columns
    };
    if !independent
        && let Some(first) = data.first()
        && data
            .iter()
            .any(|column| column.values.len() != first.values.len())
    {
        return Err(KernelError::ShapeMismatch);
    }
    inv.check_control()?;
    Ok(data)
}

fn numeric(column: &series::Column, inv: &KernelInvocation<'_>) -> Result<Vec<f64>, KernelError> {
    let mut values = inv.control.reserve(column.values.len())?;
    for (i, value) in column.values.iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        values.push(numeric_input(Some(&RuntimeValue::Scalar(value.clone())))?);
    }
    Ok(values)
}

fn strings(
    column: &series::Column,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<String>, KernelError> {
    let mut values = inv.control.reserve(column.values.len())?;
    for (i, value) in column.values.iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        values.push(match value {
            TabularScalar::String(value) => value.to_string(),
            _ => return Err(KernelError::InvalidNumericInput),
        });
    }
    Ok(values)
}

fn labels(inv: &KernelInvocation<'_>) -> Vec<String> {
    inv.inputs
        .iter()
        .enumerate()
        .map(|(i, value)| match value.unannotated() {
            RuntimeValue::Series(series) => series.plan().field().name().clone(),
            _ => format!("Series {}", i + 1),
        })
        .collect()
}

fn truth(column: &series::Column, inv: &KernelInvocation<'_>) -> Result<Vec<bool>, KernelError> {
    let meaning = column.metadata.as_ref().map(|metadata| &metadata.semantic);
    let mut result = inv.control.reserve(column.values.len())?;
    for (index, value) in column.values.iter().enumerate() {
        if index.is_multiple_of(1024) {
            inv.check_control()?;
        }
        result.push(
            if let Some(meaning) = meaning
                && let Some(positive) = &meaning.positive_value
            {
                let code = match value {
                    TabularScalar::String(value) => value.to_string(),
                    TabularScalar::Bool(value) => value.to_string(),
                    TabularScalar::Integer(value) => value.to_string(),
                    TabularScalar::Unsigned(value) => value.to_string(),
                    TabularScalar::Float64(value) => value.as_f64().to_string(),
                    TabularScalar::Null => return Err(KernelError::InvalidNumericInput),
                };
                if !meaning.values.iter().any(|value| value.value == code) {
                    return Err(KernelError::InvalidNumericInput);
                }
                code == *positive
            } else {
                match value {
                    TabularScalar::Bool(value) => *value,
                    TabularScalar::Integer(0) | TabularScalar::Unsigned(0) => false,
                    TabularScalar::Integer(1) | TabularScalar::Unsigned(1) => true,
                    TabularScalar::Float64(value) if value.as_f64() == 0.0 => false,
                    TabularScalar::Float64(value) if value.as_f64() == 1.0 => true,
                    _ => return Err(KernelError::InvalidNumericInput),
                }
            },
        );
    }
    Ok(result)
}

fn execute(method: Method, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    use Method::*;
    let control = ScientificExecutionControl::from_shared(
        inv.control.cancellation.clone(),
        inv.control.deadline,
    );
    let output = if matches!(method, Coefficients) {
        let Some(RuntimeValue::LinearRegression(model)) = inv.inputs.first() else {
            return Err(KernelError::InvalidNumericInput);
        };
        let include_intercept = boolean(inv, "include_intercept")?;
        let coefficients = &model.report.coefficients;
        let skip = usize::from(model.constant && !include_intercept);
        let selected = coefficients
            .get(skip..)
            .ok_or(KernelError::InvalidNumericInput)?;
        encode(
            sci::coefficients(
                &selected
                    .iter()
                    .map(|v| v.variable.clone())
                    .collect::<Vec<_>>(),
                &selected.iter().map(|v| v.coef).collect::<Vec<_>>(),
                &selected.iter().map(|v| v.std_err).collect::<Vec<_>>(),
                model.report.model_basic_info.df_residual as f64,
                number(inv, "confidence_level")?,
                &control,
            ),
            inv,
        )?
    } else {
        let data = prepared(inv, matches!(method, Box | Violin))?;
        if data.is_empty() {
            return Err(KernelError::InvalidNumericInput);
        }
        match method {
            WordCloud => encode(
                sci::word_cloud(
                    &strings(&data[0], inv)?,
                    integer(inv, "max_words")?,
                    &control,
                ),
                inv,
            )?,
            Pareto => encode(sci::pareto(&strings(&data[0], inv)?, &control), inv)?,
            Combination => encode(
                sci::combination(
                    &strings(&data[0], inv)?,
                    &numeric(&data[1], inv)?,
                    &numeric(&data[2], inv)?,
                    boolean(inv, "dual_axis")?,
                    &control,
                ),
                inv,
            )?,
            Roc => {
                let truth = truth(&data[0], inv)?;
                encode(sci::roc(&truth, &numeric(&data[1], inv)?, &control), inv)?
            }
            _ => {
                let columns = data
                    .iter()
                    .map(|column| numeric(column, inv))
                    .collect::<Result<Vec<_>, _>>()?;
                inv.control.check_bytes(
                    columns
                        .iter()
                        .try_fold(0usize, |n, v| n.checked_add(v.len().checked_mul(64)?)),
                )?;
                match method {
                    Scatter | Line => encode(
                        sci::xy(&columns[0], &columns[1], matches!(method, Line), &control),
                        inv,
                    )?,
                    Ecdf => encode(sci::ecdf(&columns[0], &control), inv)?,
                    Kde => encode(
                        sci::kde(&columns[0], integer(inv, "grid_points")?, &control),
                        inv,
                    )?,
                    Histogram => encode(
                        sci::histogram(&columns[0], integer(inv, "bins")?, &control),
                        inv,
                    )?,
                    Correlation => encode(sci::correlation(&labels(inv), &columns, &control), inv)?,
                    Correlogram => encode(
                        sci::correlogram(&columns[0], integer(inv, "maximum_lag")?, &control),
                        inv,
                    )?,
                    Box | Violin => encode(
                        sci::box_violin(&labels(inv), &columns, matches!(method, Violin), &control),
                        inv,
                    )?,
                    ErrorBars => encode(
                        sci::error_bars(
                            &columns[0],
                            &columns[1],
                            &columns[2],
                            &columns[3],
                            &control,
                        ),
                        inv,
                    )?,
                    Probability => encode(
                        sci::probability(
                            &columns[0],
                            match text(inv, "mode")? {
                                "pp" => ProbabilityPlotMode::Pp,
                                "qq" => ProbabilityPlotMode::Qq,
                                _ => return Err(KernelError::InvalidParameter),
                            },
                            boolean(inv, "estimate_parameters")?,
                            number(inv, "reference_mean")?,
                            number(inv, "reference_standard_deviation")?,
                            &control,
                        ),
                        inv,
                    )?,
                    Quadrant => encode(
                        sci::quadrant(
                            &columns[0],
                            &columns[1],
                            number(inv, "x_cut")?,
                            number(inv, "y_cut")?,
                            &control,
                        ),
                        inv,
                    )?,
                    Bubble => encode(
                        sci::bubble(&columns[0], &columns[1], &columns[2], &control),
                        inv,
                    )?,
                    Heatmap => encode(sci::heatmap(&labels(inv), &columns, &control), inv)?,
                    _ => unreachable!(),
                }
            }
        }
    };
    inv.check_control()?;
    Ok(vec![output])
}

#[cfg(test)]
mod tests;
