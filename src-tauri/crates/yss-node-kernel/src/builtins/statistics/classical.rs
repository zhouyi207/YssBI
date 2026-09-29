//! Kernel adapters for sample-mean hypothesis tests.
use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use std::mem::size_of;
use yss_data_contract::TabularScalar;
use yss_sci_contract::hypothesis::{
    Alternative, CategoricalHypothesisTest, ClassicalHypothesisTest, RankHypothesisTest,
    VarianceHomogeneityTest,
};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.statistics.test.t.one_sample",
        vec![Input::fixed("series")],
        &["null_mean", "alternative"],
        2,
        |inv| {
            let values: Vec<f64> = columns(&group(inv, "series"), inv, 0)?.remove(0);
            execute(
                ClassicalHypothesisTest::OneSample {
                    values,
                    null_mean: number(inv, "null_mean")?,
                    alternative: alternative(inv)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.t.independent",
        vec![Input::fixed("group1"), Input::fixed("group2")],
        &["alternative", "equal_variance"],
        2,
        |inv| {
            let first = columns(&group(inv, "group1"), inv, 0)?.remove(0);
            let second = columns(&group(inv, "group2"), inv, 0)?.remove(0);
            execute(
                ClassicalHypothesisTest::Independent {
                    first,
                    second,
                    equal_variance: boolean(inv, "equal_variance")?,
                    alternative: alternative(inv)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.t.paired",
        vec![Input::fixed("before"), Input::fixed("after")],
        &["alternative"],
        2,
        |inv| {
            let before = columns(&group(inv, "before"), inv, 0)?.remove(0);
            let after = columns(&group(inv, "after"), inv, 0)?.remove(0);
            execute(
                ClassicalHypothesisTest::Paired {
                    before,
                    after,
                    alternative: alternative(inv)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.t.summary_input",
        vec![Input::fixed("series")],
        &["design", "null_value", "alternative", "equal_variance"],
        2,
        |inv| {
            let values: Vec<f64> = columns(&group(inv, "series"), inv, 0)?.remove(0);
            let design = match text(inv, "design")? {
                "one_sample" => yss_sci_contract::hypothesis::SummaryTDesign::OneSample,
                "paired" => yss_sci_contract::hypothesis::SummaryTDesign::Paired,
                "independent" => yss_sci_contract::hypothesis::SummaryTDesign::Independent,
                _ => return Err(KernelError::InvalidParameter),
            };
            let needed = if matches!(
                design,
                yss_sci_contract::hypothesis::SummaryTDesign::Independent
            ) {
                6
            } else {
                3
            };
            if values.len() != needed {
                return Err(KernelError::ShapeMismatch);
            }
            let count = |index: usize| -> Result<usize, KernelError> {
                let value = values[index];
                if value >= 2.0 && value.fract() == 0.0 && value <= usize::MAX as f64 {
                    Ok(value as usize)
                } else {
                    Err(KernelError::InvalidParameter)
                }
            };
            execute(
                ClassicalHypothesisTest::Summary {
                    design,
                    first_count: count(0)?,
                    first_mean: values[1],
                    first_sd: values[2],
                    second_count: (needed == 6).then(|| count(3)).transpose()?,
                    second_mean: (needed == 6).then_some(values.get(4).copied()).flatten(),
                    second_sd: (needed == 6).then_some(values.get(5).copied()).flatten(),
                    null_difference: number(inv, "null_value")?,
                    equal_variance: boolean(inv, "equal_variance")?,
                    alternative: alternative(inv)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.z.mean",
        vec![Input::fixed("series")],
        &["null_mean", "population_sd", "alternative"],
        2,
        |inv| {
            let values = columns(&group(inv, "series"), inv, 0)?.remove(0);
            execute(
                ClassicalHypothesisTest::OneSampleZ {
                    values,
                    null_mean: number(inv, "null_mean")?,
                    population_sd: number(inv, "population_sd")?,
                    alternative: alternative(inv)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.z.proportion",
        vec![Input::fixed("series")],
        &["null_probability", "alternative"],
        2,
        |inv| {
            let (successes, trials) = binary_counts(inv, "series")?;
            execute(
                ClassicalHypothesisTest::OneProportionZ {
                    successes,
                    trials,
                    null_probability: number(inv, "null_probability")?,
                    alternative: alternative(inv)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.binomial",
        vec![Input::fixed("series")],
        &["null_probability", "alternative"],
        2,
        |inv| {
            let (successes, trials) = binary_counts(inv, "series")?;
            execute(
                ClassicalHypothesisTest::ExactBinomial {
                    successes,
                    trials,
                    null_probability: number(inv, "null_probability")?,
                    alternative: alternative(inv)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.proportion.two",
        vec![Input::fixed("group1"), Input::fixed("group2")],
        &["null_difference", "alternative"],
        2,
        |inv| {
            let (first_successes, first_trials) = binary_counts(inv, "group1")?;
            let (second_successes, second_trials) = binary_counts(inv, "group2")?;
            execute(
                ClassicalHypothesisTest::TwoProportions {
                    first_successes,
                    first_trials,
                    second_successes,
                    second_trials,
                    null_difference: number(inv, "null_difference")?,
                    alternative: alternative(inv)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.chisquare.crosstab",
        vec![Input::fixed("row"), Input::fixed("column")],
        &[],
        2,
        |inv| {
            ensure_aligned(inv, &["row", "column"])?;
            execute_categorical(
                CategoricalHypothesisTest::Independence {
                    row: category_values(inv, "row")?,
                    column: category_values(inv, "column")?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.chisquare.general",
        vec![Input::fixed("counts")],
        &["rows", "columns"],
        2,
        |inv| {
            let observed = columns(&group(inv, "counts"), inv, 0)?.remove(0);
            execute_categorical(
                CategoricalHypothesisTest::PearsonTable {
                    observed,
                    rows: integer(inv, "rows")?,
                    columns: integer(inv, "columns")?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.chisquare.goodness_of_fit",
        vec![Input::fixed("observed"), Input::fixed("expected")],
        &[],
        2,
        |inv| {
            let observed = columns(&group(inv, "observed"), inv, 0)?.remove(0);
            let expected = columns(&group(inv, "expected"), inv, 0)?.remove(0);
            execute_categorical(
                CategoricalHypothesisTest::GoodnessOfFit { observed, expected },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.fisher_exact",
        vec![Input::fixed("row"), Input::fixed("column")],
        &[],
        2,
        |inv| {
            ensure_aligned(inv, &["row", "column"])?;
            execute_categorical(
                CategoricalHypothesisTest::FisherExact {
                    row: category_values(inv, "row")?,
                    column: category_values(inv, "column")?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.mcnemar",
        vec![Input::fixed("before"), Input::fixed("after")],
        &[],
        2,
        |inv| {
            execute_categorical(
                CategoricalHypothesisTest::McNemar {
                    before: columns(&group(inv, "before"), inv, 0)?.remove(0),
                    after: columns(&group(inv, "after"), inv, 0)?.remove(0),
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.cmh",
        vec![
            Input::fixed("exposed"),
            Input::fixed("outcome"),
            Input::fixed("strata"),
        ],
        &[],
        2,
        |inv| {
            ensure_aligned(inv, &["exposed", "outcome", "strata"])?;
            execute_categorical(
                CategoricalHypothesisTest::Cmh {
                    exposed: category_binary_values(inv, "exposed")?,
                    outcome: category_binary_values(inv, "outcome")?,
                    strata: category_values(inv, "strata")?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.proportion.multiple",
        vec![Input::fixed("successes_and_trials")],
        &[],
        2,
        |inv| {
            let values = columns(&group(inv, "successes_and_trials"), inv, 0)?.remove(0);
            execute_categorical(
                CategoricalHypothesisTest::MultipleProportions {
                    successes_and_trials: values,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.poisson",
        vec![Input::fixed("series")],
        &["null_rate", "alternative"],
        2,
        |inv| {
            execute(
                ClassicalHypothesisTest::PoissonRate {
                    counts: columns(&group(inv, "series"), inv, 0)?.remove(0),
                    null_rate_per_observation: number(inv, "null_rate")?,
                    alternative: alternative(inv)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.equivalence",
        vec![Input::fixed("series")],
        &["lower_bound", "upper_bound"],
        2,
        |inv| {
            execute(
                ClassicalHypothesisTest::Equivalence {
                    values: columns(&group(inv, "series"), inv, 0)?.remove(0),
                    lower_bound: number(inv, "lower_bound")?,
                    upper_bound: number(inv, "upper_bound")?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.wilcoxon.one_sample",
        vec![Input::fixed("series")],
        &["null_median", "alternative"],
        2,
        |inv| {
            execute_rank(
                RankHypothesisTest::WilcoxonOneSample {
                    values: columns(&group(inv, "series"), inv, 0)?.remove(0),
                    null_median: number(inv, "null_median")?,
                    alternative: alternative(inv)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.wilcoxon.paired",
        vec![Input::fixed("before"), Input::fixed("after")],
        &["alternative"],
        2,
        |inv| {
            let values = columns(&[group(inv, "before")[0], group(inv, "after")[0]], inv, 0)?;
            execute_rank(
                RankHypothesisTest::WilcoxonPaired {
                    before: values[0].clone(),
                    after: values[1].clone(),
                    alternative: alternative(inv)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.mann_whitney",
        vec![Input::fixed("group1"), Input::fixed("group2")],
        &["alternative"],
        2,
        |inv| {
            let values = columns(&[group(inv, "group1")[0], group(inv, "group2")[0]], inv, 0)?;
            execute_rank(
                RankHypothesisTest::MannWhitney {
                    first: values[0].clone(),
                    second: values[1].clone(),
                    alternative: alternative(inv)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.kruskal_wallis",
        vec![Input::repeated("groups", 2..=16)],
        &[],
        2,
        |inv| {
            let values = columns(&group(inv, "groups"), inv, 0)?;
            execute_rank(RankHypothesisTest::KruskalWallis { groups: values }, inv)
        },
    );
    install(
        builder,
        "yssbi.statistics.test.mood_median",
        vec![Input::repeated("groups", 2..=16)],
        &[],
        2,
        |inv| {
            let values = columns(&group(inv, "groups"), inv, 0)?;
            execute_rank(RankHypothesisTest::MoodMedian { groups: values }, inv)
        },
    );
    install(
        builder,
        "yssbi.statistics.test.friedman",
        vec![Input::repeated("conditions", 3..=16)],
        &[],
        2,
        |inv| {
            let values = columns(&group(inv, "conditions"), inv, 0)?;
            execute_rank(RankHypothesisTest::Friedman { conditions: values }, inv)
        },
    );
    install(
        builder,
        "yssbi.statistics.test.cochran_q",
        vec![Input::repeated("conditions", 3..=16)],
        &[],
        2,
        |inv| {
            let values = columns(&group(inv, "conditions"), inv, 0)?;
            execute_rank(RankHypothesisTest::CochranQ { conditions: values }, inv)
        },
    );
    install(
        builder,
        "yssbi.statistics.test.runs",
        vec![Input::fixed("series")],
        &[],
        2,
        |inv| {
            execute_rank(
                RankHypothesisTest::Runs {
                    values: columns(&group(inv, "series"), inv, 0)?.remove(0),
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.mann_kendall",
        vec![Input::fixed("series")],
        &["alternative"],
        2,
        |inv| {
            execute_rank(
                RankHypothesisTest::MannKendall {
                    values: columns(&group(inv, "series"), inv, 0)?.remove(0),
                    alternative: alternative(inv)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.nonparametric.family",
        vec![Input::repeated("groups", 2..=16)],
        &["method"],
        2,
        |inv| {
            let values = columns(&group(inv, "groups"), inv, 0)?;
            match text(inv, "method")? {
                "mann_whitney" if values.len() == 2 => execute_rank(
                    RankHypothesisTest::MannWhitney {
                        first: values[0].clone(),
                        second: values[1].clone(),
                        alternative: Alternative::TwoSided,
                    },
                    inv,
                ),
                "kruskal_wallis" if values.len() >= 2 => {
                    execute_rank(RankHypothesisTest::KruskalWallis { groups: values }, inv)
                }
                "mood_median" if values.len() >= 2 => {
                    execute_rank(RankHypothesisTest::MoodMedian { groups: values }, inv)
                }
                _ => Err(KernelError::InvalidParameter),
            }
        },
    );
    install(
        builder,
        "yssbi.statistics.test.levene",
        vec![Input::repeated("groups", 2..=16)],
        &[],
        2,
        |inv| {
            execute_variance(
                VarianceHomogeneityTest::Levene {
                    groups: columns(&group(inv, "groups"), inv, 0)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.brown_forsythe",
        vec![Input::repeated("groups", 2..=16)],
        &[],
        2,
        |inv| {
            execute_variance(
                VarianceHomogeneityTest::BrownForsythe {
                    groups: columns(&group(inv, "groups"), inv, 0)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.bartlett",
        vec![Input::repeated("groups", 2..=16)],
        &[],
        2,
        |inv| {
            execute_variance(
                VarianceHomogeneityTest::Bartlett {
                    groups: columns(&group(inv, "groups"), inv, 0)?,
                },
                inv,
            )
        },
    );
}

fn alternative(inv: &KernelInvocation<'_>) -> Result<Alternative, KernelError> {
    match text(inv, "alternative")? {
        "two_sided" => Ok(Alternative::TwoSided),
        "greater" => Ok(Alternative::Greater),
        "less" => Ok(Alternative::Less),
        _ => Err(KernelError::InvalidParameter),
    }
}

fn execute(
    test: ClassicalHypothesisTest,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let report = yss_sci_runtime::hypothesis::sample_mean_test(test)
        .map_err(|_| KernelError::InvalidNumericInput)?;
    let report = value(report, inv)?;
    Ok(vec![report.clone(), report])
}

fn execute_categorical(
    test: CategoricalHypothesisTest,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let report = yss_sci_runtime::hypothesis::categorical_test(test)
        .map_err(|_| KernelError::InvalidNumericInput)?;
    let report = value(report, inv)?;
    Ok(vec![report.clone(), report])
}

fn execute_rank(
    test: RankHypothesisTest,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let report = yss_sci_runtime::hypothesis::rank_test(test)
        .map_err(|_| KernelError::InvalidNumericInput)?;
    let report = value(report, inv)?;
    Ok(vec![report.clone(), report])
}

fn execute_variance(
    test: VarianceHomogeneityTest,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let report = yss_sci_runtime::hypothesis::variance_test(test)
        .map_err(|_| KernelError::InvalidNumericInput)?;
    let report = value(report, inv)?;
    Ok(vec![report.clone(), report])
}

fn binary_counts(inv: &KernelInvocation<'_>, key: &str) -> Result<(usize, usize), KernelError> {
    let values = columns(&group(inv, key), inv, 0)?.remove(0);
    if values.is_empty() || values.iter().any(|value| *value != 0.0 && *value != 1.0) {
        return Err(KernelError::InvalidNumericInput);
    }
    let successes = values.iter().filter(|value| **value == 1.0).count();
    Ok((successes, values.len()))
}

fn category_binary_values(inv: &KernelInvocation<'_>, key: &str) -> Result<Vec<f64>, KernelError> {
    let categories = category_values(inv, key)?;
    if categories.iter().any(|value| {
        !matches!(
            value.as_ref(),
            "b:0"
                | "b:1"
                | "i:0"
                | "i:1"
                | "u:0"
                | "u:1"
                | "f:0000000000000000"
                | "f:3ff0000000000000"
        )
    }) {
        return Err(KernelError::InvalidNumericInput);
    }
    Ok(categories
        .iter()
        .map(|value| {
            if matches!(value.as_ref(), "b:1" | "i:1" | "u:1" | "f:3ff0000000000000") {
                1.0
            } else {
                0.0
            }
        })
        .collect())
}

fn category_values(inv: &KernelInvocation<'_>, key: &str) -> Result<Vec<Box<str>>, KernelError> {
    let value = group(inv, key)
        .into_iter()
        .next()
        .ok_or(KernelError::InvalidNumericInput)?;
    let mut categories = Vec::new();
    let mut bytes = 0usize;
    let mut append = |values: Vec<TabularScalar>| -> Result<(), KernelError> {
        for scalar in values {
            inv.check_control()?;
            let key: Box<str> = match scalar {
                TabularScalar::Bool(value) => format!("b:{}", u8::from(value)).into(),
                TabularScalar::Integer(value) => format!("i:{value}").into(),
                TabularScalar::Unsigned(value) => format!("u:{value}").into(),
                TabularScalar::Float64(value) => {
                    format!("f:{:016x}", value.as_f64().to_bits()).into()
                }
                TabularScalar::String(value) => format!("s:{value}").into(),
                TabularScalar::Null => return Err(KernelError::InvalidNumericInput),
            };
            bytes = bytes
                .checked_add(key.len() + size_of::<Box<str>>())
                .ok_or(KernelError::BudgetExceeded)?;
            inv.control.check_bytes(Some(bytes))?;
            categories.push(key);
        }
        Ok(())
    };
    match value {
        RuntimeValue::List(values) => {
            let scalars = values
                .iter()
                .map(|value| match value.unannotated() {
                    RuntimeValue::Scalar(value) => Ok(value.clone()),
                    _ => Err(KernelError::InvalidNumericInput),
                })
                .collect::<Result<Vec<_>, _>>()?;
            append(scalars)?;
        }
        RuntimeValue::Series(series) => {
            let relation = series
                .as_relation()
                .map_err(|_| KernelError::InvalidNumericInput)?;
            relation
                .visit_batches(&inv.relation_control(), &mut |batch| {
                    let values = yss_database_arrow::materialized_values(batch.column(0).as_ref())
                        .map_err(|_| yss_relational_contract::RelationError::InvalidInput)?;
                    append(values).map_err(|_| yss_relational_contract::RelationError::InvalidInput)
                })
                .map_err(super::super::relational::kernel_error)?;
        }
        _ => return Err(KernelError::InvalidNumericInput),
    }
    Ok(categories)
}

fn ensure_aligned(inv: &KernelInvocation<'_>, keys: &[&str]) -> Result<(), KernelError> {
    let values = keys
        .iter()
        .map(|key| {
            group(inv, key)
                .into_iter()
                .next()
                .ok_or(KernelError::InvalidNumericInput)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(RuntimeValue::Series(first)) = values.first() {
        let mut series = Vec::with_capacity(values.len());
        for value in &values {
            let RuntimeValue::Series(value) = value else {
                return Err(KernelError::UnalignedSeries);
            };
            series.push(value.clone());
        }
        first
            .relation()
            .project_series(&series)
            .map_err(|_| KernelError::UnalignedSeries)?;
        return Ok(());
    }
    let mut length = None;
    for value in values {
        let RuntimeValue::List(values) = value else {
            return Err(KernelError::UnalignedSeries);
        };
        if length.is_some_and(|expected| expected != values.len()) {
            return Err(KernelError::ShapeMismatch);
        }
        length = Some(values.len());
    }
    Ok(())
}
