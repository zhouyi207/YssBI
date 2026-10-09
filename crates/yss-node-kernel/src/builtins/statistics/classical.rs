//! Kernel adapters for sample-mean hypothesis tests.
use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use std::collections::BTreeSet;
use std::mem::size_of;
use yss_data_contract::TabularScalar;
use yss_sci_contract::execution::ScientificExecutionControl;
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
        1,
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
        1,
        |inv| {
            let first = columns(&group(inv, "group1"), inv, 0)?.remove(0);
            let retained = inv
                .control
                .check_bytes(first.len().checked_mul(size_of::<f64>()))?;
            let second = columns(&group(inv, "group2"), inv, retained)?.remove(0);
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
        1,
        |inv| {
            let mut values = columns(&[group(inv, "before")[0], group(inv, "after")[0]], inv, 0)?;
            let after = values.pop().expect("after column");
            let before = values.pop().expect("before column");
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
        1,
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
        1,
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
        1,
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
        1,
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
        1,
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
        1,
        |inv| {
            let row = category_values(inv, "row", 0)?;
            let column = category_values(inv, "column", category_bytes(&row, inv)?)?;
            if row.len() != column.len() {
                return Err(KernelError::ShapeMismatch);
            }
            execute_categorical(CategoricalHypothesisTest::Independence { row, column }, inv)
        },
    );
    install(
        builder,
        "yssbi.statistics.test.chisquare.general",
        vec![Input::fixed("counts")],
        &["rows", "columns"],
        1,
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
        1,
        |inv| {
            let observed = columns(&group(inv, "observed"), inv, 0)?.remove(0);
            let retained = inv
                .control
                .check_bytes(observed.len().checked_mul(size_of::<f64>()))?;
            let expected = columns(&group(inv, "expected"), inv, retained)?.remove(0);
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
        1,
        |inv| {
            let row = category_values(inv, "row", 0)?;
            let column = category_values(inv, "column", category_bytes(&row, inv)?)?;
            if row.len() != column.len() {
                return Err(KernelError::ShapeMismatch);
            }
            execute_categorical(CategoricalHypothesisTest::FisherExact { row, column }, inv)
        },
    );
    install(
        builder,
        "yssbi.statistics.test.mcnemar",
        vec![Input::fixed("before"), Input::fixed("after")],
        &[],
        1,
        |inv| {
            let mut values = columns(&[group(inv, "before")[0], group(inv, "after")[0]], inv, 0)?;
            let after = values.pop().expect("after column");
            let before = values.pop().expect("before column");
            execute_categorical(CategoricalHypothesisTest::McNemar { before, after }, inv)
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
        1,
        |inv| {
            let exposed = binary_values(inv, "exposed", 0)?;
            let retained = inv
                .control
                .check_bytes(exposed.len().checked_mul(size_of::<f64>()))?;
            let outcome = binary_values(inv, "outcome", retained)?;
            let retained = inv.control.check_bytes(
                exposed
                    .len()
                    .checked_add(outcome.len())
                    .and_then(|n| n.checked_mul(size_of::<f64>())),
            )?;
            let strata = category_values(inv, "strata", retained)?;
            if exposed.len() != outcome.len() || outcome.len() != strata.len() {
                return Err(KernelError::ShapeMismatch);
            }
            execute_categorical(
                CategoricalHypothesisTest::Cmh {
                    exposed,
                    outcome,
                    strata,
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
        1,
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
        1,
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
        1,
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
        1,
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
        1,
        |inv| {
            let mut values = columns(&[group(inv, "before")[0], group(inv, "after")[0]], inv, 0)?;
            let after = values.pop().expect("second column");
            let before = values.pop().expect("first column");
            execute_rank(
                RankHypothesisTest::WilcoxonPaired {
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
        "yssbi.statistics.test.mann_whitney",
        vec![Input::fixed("group1"), Input::fixed("group2")],
        &["alternative"],
        1,
        |inv| {
            let mut values =
                independent_columns(&[group(inv, "group1")[0], group(inv, "group2")[0]], inv, 0)?;
            let second = values.pop().expect("second column");
            let first = values.pop().expect("first column");
            execute_rank(
                RankHypothesisTest::MannWhitney {
                    first,
                    second,
                    alternative: alternative(inv)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.kruskal_wallis",
        vec![Input::repeated("groups", 2..=usize::MAX)],
        &[],
        1,
        |inv| {
            let values = independent_columns(&group(inv, "groups"), inv, 0)?;
            execute_rank(RankHypothesisTest::KruskalWallis { groups: values }, inv)
        },
    );
    install(
        builder,
        "yssbi.statistics.test.mood_median",
        vec![Input::repeated("groups", 2..=usize::MAX)],
        &[],
        1,
        |inv| {
            let values = independent_columns(&group(inv, "groups"), inv, 0)?;
            execute_rank(RankHypothesisTest::MoodMedian { groups: values }, inv)
        },
    );
    install(
        builder,
        "yssbi.statistics.test.friedman",
        vec![Input::repeated("conditions", 3..=usize::MAX)],
        &[],
        1,
        |inv| {
            let values = columns(&group(inv, "conditions"), inv, 0)?;
            execute_rank(RankHypothesisTest::Friedman { conditions: values }, inv)
        },
    );
    install(
        builder,
        "yssbi.statistics.test.cochran_q",
        vec![Input::repeated("conditions", 3..=usize::MAX)],
        &[],
        1,
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
        1,
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
        1,
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
        vec![Input::repeated("groups", 2..=usize::MAX)],
        &["method"],
        1,
        |inv| {
            let values = independent_columns(&group(inv, "groups"), inv, 0)?;
            match text(inv, "method")? {
                "mann_whitney" if values.len() == 2 => {
                    let mut values = values;
                    let second = values.pop().expect("second column");
                    let first = values.pop().expect("first column");
                    execute_rank(
                        RankHypothesisTest::MannWhitney {
                            first,
                            second,
                            alternative: Alternative::TwoSided,
                        },
                        inv,
                    )
                }
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
        vec![Input::repeated("groups", 2..=usize::MAX)],
        &[],
        1,
        |inv| {
            execute_variance(
                VarianceHomogeneityTest::Levene {
                    groups: independent_columns(&group(inv, "groups"), inv, 0)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.brown_forsythe",
        vec![Input::repeated("groups", 2..=usize::MAX)],
        &[],
        1,
        |inv| {
            execute_variance(
                VarianceHomogeneityTest::BrownForsythe {
                    groups: independent_columns(&group(inv, "groups"), inv, 0)?,
                },
                inv,
            )
        },
    );
    install(
        builder,
        "yssbi.statistics.test.bartlett",
        vec![Input::repeated("groups", 2..=usize::MAX)],
        &[],
        1,
        |inv| {
            execute_variance(
                VarianceHomogeneityTest::Bartlett {
                    groups: independent_columns(&group(inv, "groups"), inv, 0)?,
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
    let observations = match &test {
        ClassicalHypothesisTest::OneSample { values, .. }
        | ClassicalHypothesisTest::OneSampleZ { values, .. }
        | ClassicalHypothesisTest::Equivalence { values, .. } => Some(values.len()),
        ClassicalHypothesisTest::PoissonRate { counts, .. } => Some(counts.len()),
        ClassicalHypothesisTest::Independent { first, second, .. } => {
            first.len().checked_add(second.len())
        }
        ClassicalHypothesisTest::Paired { before, after, .. } => {
            before.len().checked_add(after.len())
        }
        _ => Some(0),
    };
    numeric_workspace(observations, 2, inv)?;
    let control = scientific_control(inv);
    let report =
        yss_sci_runtime::hypothesis::sample_mean_test(test, &control).map_err(computation_error)?;
    let report = value(report, inv)?;
    Ok(vec![report])
}

fn execute_categorical(
    test: CategoricalHypothesisTest,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    match &test {
        CategoricalHypothesisTest::Independence { row, column }
        | CategoricalHypothesisTest::FisherExact { row, column } => {
            count_table_workspace(row, column, inv)?
        }
        CategoricalHypothesisTest::GoodnessOfFit { observed, expected } => {
            numeric_workspace(observed.len().checked_add(expected.len()), 2, inv)?
        }
        CategoricalHypothesisTest::PearsonTable { observed, .. } => {
            numeric_workspace(Some(observed.len()), 2, inv)?
        }
        CategoricalHypothesisTest::McNemar { before, after } => {
            numeric_workspace(before.len().checked_add(after.len()), 2, inv)?
        }
        CategoricalHypothesisTest::MultipleProportions {
            successes_and_trials,
        } => numeric_workspace(Some(successes_and_trials.len()), 2, inv)?,
        CategoricalHypothesisTest::Cmh {
            exposed,
            outcome,
            strata,
        } => {
            let labels = category_bytes(strata, inv)?;
            inv.control.check_bytes(
                exposed
                    .len()
                    .checked_add(outcome.len())
                    .and_then(|n| n.checked_mul(size_of::<f64>()))
                    .and_then(|n| n.checked_add(labels))
                    .and_then(|n| n.checked_add(strata.len().checked_mul(160)?))
                    .and_then(|n| n.checked_add(4096)),
            )?;
        }
    }
    let control = scientific_control(inv);
    let report =
        yss_sci_runtime::hypothesis::categorical_test(test, &control).map_err(computation_error)?;
    let report = value(report, inv)?;
    Ok(vec![report])
}

fn category_bytes(values: &[Box<str>], inv: &KernelInvocation<'_>) -> Result<usize, KernelError> {
    let mut bytes = 0usize;
    for (i, key) in values.iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        bytes = inv.control.check_bytes(
            bytes
                .checked_add(size_of::<Box<str>>() * 2)
                .and_then(|n| n.checked_add(key.len())),
        )?;
    }
    Ok(bytes)
}

fn count_table_workspace(
    row: &[Box<str>],
    column: &[Box<str>],
    inv: &KernelInvocation<'_>,
) -> Result<(), KernelError> {
    let retained = inv
        .control
        .check_bytes(category_bytes(row, inv)?.checked_add(category_bytes(column, inv)?))?;
    // Admit the borrowed cardinality indexes before allocating them. The SCI stage
    // also retains cloned labels and tree indexes; this is a conservative estimate, not RSS.
    const INDEX_BYTES: usize = 128;
    inv.control.check_bytes(
        row.len()
            .checked_add(column.len())
            .and_then(|n| n.checked_mul(INDEX_BYTES))
            .and_then(|n| n.checked_add(retained)),
    )?;
    let mut rows = BTreeSet::new();
    let mut columns = BTreeSet::new();
    for (values, levels) in [(row, &mut rows), (column, &mut columns)] {
        for (i, key) in values.iter().enumerate() {
            if i.is_multiple_of(1024) {
                inv.check_control()?;
            }
            levels.insert(key.as_ref());
        }
    }
    inv.control.check_bytes((|| {
        let levels = rows.len().checked_add(columns.len())?;
        let table = rows
            .len()
            .checked_mul(columns.len())?
            .checked_mul(size_of::<u64>())?;
        retained
            .checked_mul(2)?
            .checked_add(levels.checked_mul(INDEX_BYTES + size_of::<f64>())?)?
            .checked_add(rows.len().checked_mul(size_of::<Vec<u64>>())?)?
            .checked_add(table)?
            .checked_add(4096)
    })())?;
    Ok(())
}

fn execute_rank(
    test: RankHypothesisTest,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let (observations, groups) = match &test {
        RankHypothesisTest::WilcoxonOneSample { values, .. }
        | RankHypothesisTest::Runs { values }
        | RankHypothesisTest::MannKendall { values, .. } => (Some(values.len()), 1),
        RankHypothesisTest::WilcoxonPaired { before, after, .. } => {
            (before.len().checked_add(after.len()), 2)
        }
        RankHypothesisTest::MannWhitney { first, second, .. } => {
            (first.len().checked_add(second.len()), 2)
        }
        RankHypothesisTest::KruskalWallis { groups }
        | RankHypothesisTest::MoodMedian { groups }
        | RankHypothesisTest::Friedman { conditions: groups }
        | RankHypothesisTest::CochranQ { conditions: groups } => {
            (group_observations(groups, inv)?, groups.len())
        }
    };
    numeric_workspace(observations, groups, inv)?;
    let control = scientific_control(inv);
    let report =
        yss_sci_runtime::hypothesis::rank_test(test, &control).map_err(computation_error)?;
    let report = value(report, inv)?;
    Ok(vec![report])
}

fn execute_variance(
    test: VarianceHomogeneityTest,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let groups = match &test {
        VarianceHomogeneityTest::Levene { groups }
        | VarianceHomogeneityTest::BrownForsythe { groups }
        | VarianceHomogeneityTest::Bartlett { groups } => groups,
    };
    numeric_workspace(group_observations(groups, inv)?, groups.len(), inv)?;
    let control = scientific_control(inv);
    let report =
        yss_sci_runtime::hypothesis::variance_test(test, &control).map_err(computation_error)?;
    let report = value(report, inv)?;
    Ok(vec![report])
}

fn scientific_control(inv: &KernelInvocation<'_>) -> ScientificExecutionControl {
    ScientificExecutionControl::from_shared(inv.control.cancellation.clone(), inv.control.deadline)
}

fn group_observations(
    groups: &[Vec<f64>],
    inv: &KernelInvocation<'_>,
) -> Result<Option<usize>, KernelError> {
    let mut total = Some(0usize);
    for (i, group) in groups.iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        total = total.and_then(|n| n.checked_add(group.len()));
    }
    Ok(total)
}

fn numeric_workspace(
    observations: Option<usize>,
    groups: usize,
    inv: &KernelInvocation<'_>,
) -> Result<(), KernelError> {
    // Retained inputs, rank/order/tie arrays and stable-sort scratch, deviations,
    // per-condition rows, and structured sample-size output. Admission is conservative, not RSS.
    inv.control.check_bytes(
        observations
            .and_then(|n| n.checked_mul(128))
            .and_then(|n| n.checked_add(groups.checked_mul(512)?))
            .and_then(|n| n.checked_add(4096)),
    )?;
    Ok(())
}

fn binary_counts(inv: &KernelInvocation<'_>, key: &str) -> Result<(usize, usize), KernelError> {
    let values = columns(&group(inv, key), inv, 0)?.remove(0);
    if values.is_empty() {
        return Err(KernelError::InvalidNumericInput);
    }
    let mut successes = 0;
    for (i, value) in values.iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        if *value != 0.0 && *value != 1.0 {
            return Err(KernelError::InvalidNumericInput);
        }
        successes += usize::from(*value == 1.0);
    }
    Ok((successes, values.len()))
}

fn binary_values(
    inv: &KernelInvocation<'_>,
    key: &str,
    retained: usize,
) -> Result<Vec<f64>, KernelError> {
    let input = group(inv, key)
        .into_iter()
        .next()
        .ok_or(KernelError::InvalidNumericInput)?;
    let column =
        super::super::series::column_retaining(input, inv, retained).map_err(
            |error| match error {
                KernelError::InvalidParameter => KernelError::InvalidNumericInput,
                error => error,
            },
        )?;
    inv.control.check_bytes((|| {
        retained
            .checked_add(column.bytes()?)?
            .checked_add(column.values.len().checked_mul(size_of::<f64>())?)
    })())?;
    let values = numeric(&column, true, inv)?;
    for (i, &value) in values.iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        if value != 0.0 && value != 1.0 {
            return Err(KernelError::InvalidNumericInput);
        }
    }
    Ok(values)
}

fn category_values(
    inv: &KernelInvocation<'_>,
    key: &str,
    retained: usize,
) -> Result<Vec<Box<str>>, KernelError> {
    let value = group(inv, key)
        .into_iter()
        .next()
        .ok_or(KernelError::InvalidNumericInput)?;
    let mut categories = Vec::new();
    let mut bytes = 0usize;
    match value {
        RuntimeValue::List(values) => {
            for value in values.iter() {
                let RuntimeValue::Scalar(value) = value.unannotated() else {
                    return Err(KernelError::InvalidNumericInput);
                };
                append_category(value, &mut categories, &mut bytes, retained, 0, inv)?;
            }
        }
        RuntimeValue::Series(series) => {
            let relation = series
                .as_relation()
                .map_err(|_| KernelError::InvalidNumericInput)?;
            let mut input_error = None;
            let visited = relation.visit_batches(&inv.relation_control(), &mut |batch| {
                let result = append_category_batch(
                    batch.column(0).as_ref(),
                    &mut categories,
                    &mut bytes,
                    retained,
                    inv,
                );
                result.map_err(|error| {
                    input_error = Some(error);
                    yss_relational_contract::RelationError::InvalidInput
                })
            });
            if let Some(error) = input_error {
                return Err(error);
            }
            visited.map_err(super::super::relational::kernel_error)?;
        }
        _ => return Err(KernelError::InvalidNumericInput),
    }
    Ok(categories)
}

fn append_category(
    scalar: &TabularScalar,
    categories: &mut Vec<Box<str>>,
    bytes: &mut usize,
    retained: usize,
    temporary: usize,
    inv: &KernelInvocation<'_>,
) -> Result<(), KernelError> {
    let encoded_bytes = match scalar {
        TabularScalar::String(value) => value.len().checked_add(2),
        TabularScalar::Null => return Err(KernelError::InvalidNumericInput),
        _ => Some(32),
    };
    *bytes = inv.control.check_bytes(
        encoded_bytes
            .and_then(|n| n.checked_add(size_of::<Box<str>>() * 2))
            .and_then(|n| bytes.checked_add(n)),
    )?;
    inv.control.check_bytes(
        bytes
            .checked_add(retained)
            .and_then(|n| n.checked_add(temporary)),
    )?;
    let key: Box<str> = match scalar {
        TabularScalar::Bool(value) => format!("b:{}", u8::from(*value)).into(),
        TabularScalar::Integer(value) => format!("i:{value}").into(),
        TabularScalar::Unsigned(value) => format!("u:{value}").into(),
        TabularScalar::Float64(value) => format!("f:{:016x}", value.as_f64().to_bits()).into(),
        TabularScalar::String(value) => format!("s:{value}").into(),
        TabularScalar::Null => return Err(KernelError::InvalidNumericInput),
    };
    categories.push(key);
    Ok(())
}

pub(super) fn append_category_batch(
    array: &dyn arrow_array::Array,
    categories: &mut Vec<Box<str>>,
    bytes: &mut usize,
    retained: usize,
    inv: &KernelInvocation<'_>,
) -> Result<(), KernelError> {
    // Indirect strings can repeat one large buffer many times. Expand one row at
    // a time so physical buffer admission also covers each temporary label copy.
    let indirect = matches!(
        array.data_type(),
        arrow_schema::DataType::Dictionary(..) | arrow_schema::DataType::Utf8View
    );
    let temporary = inv.control.check_bytes(
        (if indirect { 1 } else { array.len() })
            .checked_mul(size_of::<TabularScalar>() * 2)
            .and_then(|n| n.checked_add(array.get_array_memory_size().checked_mul(4)?)),
    )?;
    let mut append =
        |array: &dyn arrow_array::Array, bytes: &mut usize| -> Result<(), KernelError> {
            inv.control.check_bytes(
                bytes
                    .checked_add(retained)
                    .and_then(|n| n.checked_add(temporary)),
            )?;
            let values = yss_database_arrow::materialized_values(array)
                .map_err(|_| KernelError::InvalidNumericInput)?;
            for value in &values {
                append_category(value, categories, bytes, retained, temporary, inv)?;
            }
            Ok(())
        };
    if indirect {
        for i in 0..array.len() {
            // Slicing shares buffers, but is still preceded by the combined admission.
            inv.control.check_bytes(
                bytes
                    .checked_add(retained)
                    .and_then(|n| n.checked_add(temporary)),
            )?;
            append(array.slice(i, 1).as_ref(), bytes)?;
        }
    } else {
        append(array, bytes)?;
    }
    Ok(())
}
