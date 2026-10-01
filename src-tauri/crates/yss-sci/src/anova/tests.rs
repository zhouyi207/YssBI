use super::*;
use std::time::{Duration, Instant};

fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-8 * expected.abs().max(1.0),
        "actual {actual}, expected {expected}"
    );
}
fn fixture() -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<Factor>) {
    let mut y = Vec::new();
    let mut z = Vec::new();
    let mut x = Vec::new();
    let mut a = Vec::new();
    let mut b = Vec::new();
    for (av, counts) in [[3, 4, 2], [4, 3, 5]].iter().enumerate() {
        for (bv, &count) in counts.iter().enumerate() {
            for _ in 0..count {
                let i = y.len();
                let noise = [-0.8, 0.2, 0.9, -0.3, 0.4, -0.6, 0.7][i % 7];
                let value =
                    3.0 + 2.0 * av as f64 - 0.7 * bv as f64 + 1.5 * (av * bv) as f64 + noise;
                y.push(value);
                z.push(value * 0.4 + 0.9 * av as f64 - 0.3 * bv as f64 + noise * noise);
                x.push((i * 7 % 17) as f64 / 3.0);
                a.push(av);
                b.push(bv);
            }
        }
    }
    (
        y,
        z,
        x,
        vec![
            Factor {
                values: a,
                levels: 2,
            },
            Factor {
                values: b,
                levels: 3,
            },
        ],
    )
}

#[test]
fn one_way_anova_matches_between_within_decomposition() {
    let result = anova(
        &[1., 2., 3., 2., 4., 6.],
        &[Factor {
            values: vec![0, 0, 0, 1, 1, 1],
            levels: 2,
        }],
        &[],
        AnovaOptions::default(),
        &control(),
    )
    .unwrap();
    close(result.table[0].sum_squares, 6.0);
    close(result.error.sum_squares, 10.0);
    close(result.total_sum_squares, 16.0);
    close(result.table[0].f_statistic, 2.4);
    assert_eq!(
        (result.table[0].df, result.error.df, result.total_df),
        (1, 4, 5)
    );
    close(result.table[0].partial_eta_squared, 0.375);
    close(result.r_squared, 0.375);
}

#[test]
fn unbalanced_anova_distinguishes_all_three_sums_of_squares() {
    // Reference: statsmodels 0.14.6 anova_lm with C(a, Sum)*C(b, Sum).
    let (y, _, _, factors) = fixture();
    for (kind, sums, p_b) in [
        (
            SumsOfSquares::TypeI,
            [67.8914285714, 0.9053781513, 9.6699551821],
            0.4144139793,
        ),
        (
            SumsOfSquares::TypeII,
            [61.5168067227, 0.9053781513, 9.6699551821],
            0.4144139793,
        ),
        (
            SumsOfSquares::TypeIII,
            [64.8607202381, 0.0054173669, 9.6699551821],
            0.9944248252,
        ),
    ] {
        let result = anova(
            &y,
            &factors,
            &[],
            AnovaOptions {
                sums_of_squares: kind,
                ..Default::default()
            },
            &control(),
        )
        .unwrap();
        let wire = serde_json::to_value(result.options).unwrap();
        assert_eq!(
            wire["sums_of_squares"],
            match kind {
                SumsOfSquares::TypeI => "type_i",
                SumsOfSquares::TypeII => "type_ii",
                SumsOfSquares::TypeIII => "type_iii",
            }
        );
        for (term, ss) in result.table.iter().zip(sums) {
            close(term.sum_squares, ss);
        }
        close(result.table[1].p_value, p_b);
        close(result.error.sum_squares, 7.2646666667);
        assert_eq!(result.error.df, 15);
    }
    let full = anova(&y, &factors, &[], AnovaOptions::default(), &control()).unwrap();
    let additive = anova(
        &y,
        &factors,
        &[],
        AnovaOptions {
            model: FactorialModel::MainEffects,
            ..Default::default()
        },
        &control(),
    )
    .unwrap();
    assert_eq!(additive.table.len(), 2);
    close(
        additive.error.sum_squares - full.error.sum_squares,
        full.table[2].sum_squares,
    );
}

#[test]
fn factorial_anova_includes_high_order_terms_in_three_and_four_factor_designs() {
    for count in [3, 4] {
        let cells = 1usize << count;
        let mut factors = (0..count)
            .map(|_| Factor {
                values: Vec::new(),
                levels: 2,
            })
            .collect::<Vec<_>>();
        let mut y = Vec::new();
        for cell in 0..cells {
            for noise in [-0.3, 0.2, 0.4, -0.3] {
                let mut value = 10.0 + noise;
                for mask in 1..cells {
                    let sign = (0..count)
                        .filter(|i| mask & (1 << i) != 0)
                        .map(|i| if cell & (1 << i) == 0 { 1.0 } else { -1.0 })
                        .product::<f64>();
                    value += mask as f64 / 10.0 * sign;
                }
                y.push(value);
                for (i, factor) in factors.iter_mut().enumerate() {
                    factor.values.push(usize::from(cell & (1 << i) != 0));
                }
            }
        }
        let result = anova(&y, &factors, &[], AnovaOptions::default(), &control()).unwrap();
        assert_eq!(result.table.len(), cells - 1);
        let last = result.table.last().unwrap();
        close(
            last.sum_squares,
            y.len() as f64 * ((cells - 1) as f64 / 10.0).powi(2),
        );
        assert_eq!(
            last.term,
            (1..=count)
                .map(|i| format!("factor{i}"))
                .collect::<Vec<_>>()
                .join(":")
        );
        close(
            result.table.iter().map(|row| row.sum_squares).sum::<f64>() + result.error.sum_squares,
            result.total_sum_squares,
        );
    }
}

#[test]
fn ancova_adjusts_for_continuous_covariates_and_is_translation_invariant() {
    let (y, _, x, factors) = fixture();
    let response = y
        .iter()
        .zip(&x)
        .map(|(y, x)| y + 1.2 * x)
        .collect::<Vec<_>>();
    let result = anova(
        &response,
        &factors,
        std::slice::from_ref(&x),
        AnovaOptions::default(),
        &control(),
    )
    .unwrap();
    assert_eq!(result.method, "ancova");
    assert_eq!(result.table[0].term, "covariate1");
    close(result.table[0].sum_squares, 81.8845495104);
    close(result.table[1].sum_squares, 64.6064954997);
    close(result.table[2].sum_squares, 0.0101711182);
    close(result.table[3].sum_squares, 9.5741716613);
    close(result.error.sum_squares, 7.1801171563);
    assert_eq!(result.error.df, 14);
    let shifted = anova(
        &response.iter().map(|x| x + 1000.0).collect::<Vec<_>>(),
        &factors,
        &[x.iter().map(|x| x + 5000.0).collect()],
        AnovaOptions::default(),
        &control(),
    )
    .unwrap();
    for (a, b) in result.table.iter().zip(&shifted.table) {
        close(a.f_statistic, b.f_statistic);
    }
    close(
        shifted.covariate_means[0] - result.covariate_means[0],
        5000.0,
    );
}

#[test]
fn manova_sscp_and_four_f_approximations_match_reference() {
    let (y, z, _, factors) = fixture();
    let result = manova(&[y, z], &factors, AnovaOptions::default(), &control()).unwrap();
    assert_eq!(result.error_df, 15);
    assert_eq!(result.table.len(), 3);
    close(result.error_sscp[0][0], 7.2646666667);
    close(result.error_sscp[0][1], 3.4716666667);
    close(result.error_sscp[1][1], 3.0633333333);
    close(result.table[2].hypothesis_sscp[0][1], 4.3493417367);
    for (row, (statistic, f, df2, p)) in result.table[2].tests.iter().zip([
        (0.4174290269, 3.8344461985, 28.0, 0.0131505494),
        (0.5966540546, 3.1887400427, 30.0, 0.0269400159),
        (1.3618791581, 4.6769797403, 15.8181818182, 0.0110148380),
        (1.3366384759, 10.0247885693, 15.0, 0.0017201391),
    ]) {
        close(row.statistic, statistic);
        close(row.f_statistic.unwrap(), f);
        close(row.df_denominator.unwrap(), df2);
        close(row.p_value.unwrap(), p);
    }
}

#[test]
fn repeated_measures_uses_subject_errors_and_greenhouse_geisser_degrees() {
    let y = [2., 4., 6., 3., 6., 7., 5., 7., 9., 4., 5., 8.];
    let subjects = (0..12).map(|i| i / 3).collect::<Vec<_>>();
    let factors = [Factor {
        values: (0..12).map(|i| i % 3).collect(),
        levels: 3,
    }];
    let result = repeated_measures(
        &y,
        &subjects,
        &factors,
        SphericityCorrection::GreenhouseGeisser,
        &control(),
    )
    .unwrap();
    let row = &result.table[0];
    close(row.f_statistic, 72.0);
    close(row.epsilon_greenhouse_geisser, 0.5);
    close(row.p_value_uncorrected, 0.000064);
    close(row.df_numerator, 1.0);
    close(row.df_denominator, 3.0);
    assert!(row.p_value > row.p_value_uncorrected);
    let uncorrected = repeated_measures(
        &y,
        &subjects,
        &factors,
        SphericityCorrection::None,
        &control(),
    )
    .unwrap();
    close(uncorrected.table[0].p_value, row.p_value_uncorrected);
    let shift = y
        .iter()
        .enumerate()
        .map(|(i, y)| y + (i / 3) as f64 * 30.0)
        .collect::<Vec<_>>();
    let shifted = repeated_measures(
        &shift,
        &subjects,
        &factors,
        SphericityCorrection::GreenhouseGeisser,
        &control(),
    )
    .unwrap();
    close(shifted.table[0].f_statistic, row.f_statistic);
}

#[test]
fn manova_zero_effects_and_unavailable_small_sample_inference_remain_explicit() {
    let result = manova(
        &[
            vec![-1., 0., 1., -1., 0., 1.],
            vec![1., -2., 1., 1., -2., 1.],
        ],
        &[Factor {
            values: vec![0, 0, 0, 1, 1, 1],
            levels: 2,
        }],
        AnovaOptions::default(),
        &control(),
    )
    .unwrap();
    for test in &result.table[0].tests {
        close(test.p_value.unwrap(), 1.0);
        close(test.f_statistic.unwrap(), 0.0);
    }
    close(result.table[0].tests[0].statistic, 1.0);
    let result = manova(
        &[
            vec![1., 2., 3., 4., 0., -1., 2.],
            vec![0., 1., 2., 1., 3., 5., 4.],
            vec![1., 0., 1., -1., 2., 2., 3.],
        ],
        &[Factor {
            values: vec![0, 1, 2, 3, 3, 3, 3],
            levels: 4,
        }],
        AnovaOptions::default(),
        &control(),
    )
    .unwrap();
    let hotelling = &result.table[0].tests[2];
    assert!(hotelling.statistic.is_finite());
    assert!(
        hotelling.f_statistic.is_none()
            && hotelling.df_numerator.is_none()
            && hotelling.df_denominator.is_none()
            && hotelling.p_value.is_none()
    );
    assert!(result.table[0].tests[0].p_value.is_some());
}

#[test]
fn repeated_factorial_contrasts_match_two_way_subject_interaction_tests() {
    let mut y = Vec::new();
    let mut subjects = Vec::new();
    let mut a = Vec::new();
    let mut b = Vec::new();
    for s in 0..6 {
        for av in 0..2 {
            for bv in 0..3 {
                let i = y.len();
                y.push(
                    1.0 + s as f64 * 0.7
                        + av as f64 * 2.0
                        + bv as f64 * 0.4
                        + (av * bv) as f64 * 0.8
                        + ((i * 7 % 13) as f64 - 6.0) * 0.09,
                );
                subjects.push(s);
                a.push(av);
                b.push(bv);
            }
        }
    }
    let factors = [
        Factor {
            values: a,
            levels: 2,
        },
        Factor {
            values: b,
            levels: 3,
        },
    ];
    let result = repeated_measures(
        &y,
        &subjects,
        &factors,
        SphericityCorrection::GreenhouseGeisser,
        &control(),
    )
    .unwrap();
    for (row, f) in result
        .table
        .iter()
        .zip([1132.7054569362, 200.3333333333, 9.9698165083])
    {
        close(row.f_statistic, f);
    }
    assert_eq!((result.table[0].df, result.table[0].error_df), (1, 5));
    assert_eq!((result.table[2].df, result.table[2].error_df), (2, 10));
    close(result.table[0].epsilon_greenhouse_geisser, 1.0);
}

#[test]
fn anova_rejects_singular_incomplete_nonfinite_and_out_of_bounds_designs() {
    let (y, z, _, factors) = fixture();
    assert!(
        anova(
            &y,
            &[factors[0].clone(), factors[0].clone()],
            &[],
            AnovaOptions::default(),
            &control()
        )
        .is_err()
    );
    assert!(
        anova(
            &y,
            &factors,
            &[vec![1.0; y.len()]],
            AnovaOptions::default(),
            &control()
        )
        .is_err()
    );
    assert!(
        manova(
            &[y.clone(), y.clone()],
            &factors,
            AnovaOptions::default(),
            &control()
        )
        .is_err()
    );
    assert!(
        manova(
            &[y.clone(), z[..z.len() - 1].to_vec()],
            &factors,
            AnovaOptions::default(),
            &control()
        )
        .is_err()
    );
    assert!(
        anova(
            &vec![5.0; y.len()],
            &factors,
            &[],
            AnovaOptions::default(),
            &control()
        )
        .is_err()
    );
    let mut nonfinite = y.clone();
    nonfinite[0] = f64::NAN;
    assert!(matches!(
        anova(
            &nonfinite,
            &factors,
            &[],
            AnovaOptions::default(),
            &control()
        ),
        Err(ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::NonFiniteInput
        })
    ));
    assert_eq!(
        design_columns(&[17, 17], 0, FactorialModel::FullFactorial),
        Some(289)
    );
    let within = [Factor {
        values: vec![0, 1, 0, 1],
        levels: 2,
    }];
    assert!(
        repeated_measures(
            &[1., 3., 2., 5.],
            &[0, 0, 0, 1],
            &within,
            SphericityCorrection::None,
            &control()
        )
        .is_err()
    );
    let duplicate = [Factor {
        values: vec![0, 0, 0, 1],
        levels: 2,
    }];
    assert!(
        repeated_measures(
            &[1., 3., 2., 5.],
            &[0, 0, 1, 1],
            &duplicate,
            SphericityCorrection::None,
            &control()
        )
        .is_err()
    );
}

#[test]
fn anova_entries_preserve_cancellation_and_deadline_failures() {
    let (y, z, _, factors) = fixture();
    let cancelled = control();
    cancelled.cancellation.cancel();
    let expired = ScientificExecutionControl {
        deadline: Instant::now(),
        ..control()
    };
    for (control, expected) in [
        (cancelled, ScientificComputationError::Cancelled),
        (expired, ScientificComputationError::DeadlineExceeded),
    ] {
        assert_eq!(
            anova(&y, &factors, &[], AnovaOptions::default(), &control).unwrap_err(),
            expected
        );
        assert_eq!(
            manova(
                &[y.clone(), z.clone()],
                &factors,
                AnovaOptions::default(),
                &control
            )
            .unwrap_err(),
            expected
        );
        assert_eq!(
            repeated_measures(
                &y,
                &vec![0; y.len()],
                &factors,
                SphericityCorrection::None,
                &control
            )
            .unwrap_err(),
            expected
        );
    }
}

#[test]
fn scale_limits_anova_uses_dynamic_terms_and_repeated_cells() {
    let n = 128usize;
    let factors = (1..=70)
        .map(|j| Factor {
            values: (0..n).map(|i| (i & j).count_ones() as usize % 2).collect(),
            levels: 2,
        })
        .collect::<Vec<_>>();
    let y = (0..n)
        .map(|i| {
            1.0 + 2.0 * (1.0 - 2.0 * factors[0].values[i] as f64)
                + if (i & 127).count_ones().is_multiple_of(2) {
                    0.2
                } else {
                    -0.2
                }
        })
        .collect::<Vec<_>>();
    let result = anova(
        &y,
        &factors,
        &[],
        AnovaOptions {
            model: FactorialModel::MainEffects,
            sums_of_squares: SumsOfSquares::TypeII,
        },
        &control(),
    )
    .unwrap();
    assert_eq!(result.table.len(), 70);
    assert_eq!(result.table[69].term, "factor70");
    close(result.table[0].f_statistic, 5700.0);
    assert_eq!(result.error.df, 57);

    let cells = 512usize;
    let subjects = (0..3)
        .flat_map(|i| std::iter::repeat_n(i, cells))
        .collect::<Vec<_>>();
    let factors = (0..9)
        .map(|j| Factor {
            values: (0..subjects.len()).map(|i| (i % cells >> j) & 1).collect(),
            levels: 2,
        })
        .collect::<Vec<_>>();
    let mut state = 37u64;
    let y = (0..subjects.len())
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            (state >> 32) as f64 / u32::MAX as f64
        })
        .collect::<Vec<_>>();
    let result = repeated_measures(
        &y,
        &subjects,
        &factors,
        SphericityCorrection::None,
        &control(),
    )
    .unwrap();
    assert_eq!(result.cells_per_subject, cells);
    assert_eq!(result.table.len(), cells - 1);
    assert!(result.table.iter().all(|term| term.p_value.is_finite()));
    assert!(design_columns(&[usize::MAX, 2], 0, FactorialModel::FullFactorial).is_none());
}
