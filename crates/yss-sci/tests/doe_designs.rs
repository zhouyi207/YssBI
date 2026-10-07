use std::collections::BTreeSet;
use std::time::{Duration, Instant};
use yss_sci::doe::{design_dimensions, generate_design};
use yss_sci_contract::{doe::DesignSpecification as Spec, execution::*};
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: Default::default(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}

#[test]
fn factorial_enumeration_and_orthogonal_balance_cover_prime_and_composite_levels() {
    let c = control();
    let result = generate_design(
        Spec::FullFactorial {
            factors: 10,
            levels: 2,
        },
        &c,
    )
    .unwrap();
    assert_eq!(result.rows.len(), 1024);
    assert_eq!(result.rows[1023], [&[1024.][..], &[2.; 10]].concat());
    let distinct: BTreeSet<Vec<_>> = result
        .rows
        .iter()
        .map(|r| r[1..].iter().map(|&v| v as usize).collect())
        .collect();
    assert_eq!(distinct.len(), 1024);
    for (factors, levels, runs) in [(20, 2, 32), (4, 3, 9), (6, 5, 25), (3, 4, 64)] {
        let result = generate_design(Spec::Orthogonal { factors, levels }, &c).unwrap();
        assert_eq!(result.rows.len(), runs);
        for a in 1..=factors {
            for b in 1..a {
                let mut counts = vec![0; levels * levels];
                for row in &result.rows {
                    counts[(row[a] as usize - 1) * levels + row[b] as usize - 1] += 1;
                }
                assert!(counts.iter().all(|&n| n == runs / (levels * levels)));
            }
        }
    }
    assert!(
        design_dimensions(
            Spec::FullFactorial {
                factors: usize::MAX,
                levels: 2
            },
            &c
        )
        .is_err()
    );
    assert!(
        design_dimensions(
            Spec::Orthogonal {
                factors: 3,
                levels: 1
            },
            &c
        )
        .is_err()
    );
    c.cancellation.cancel();
    assert!(matches!(
        generate_design(
            Spec::FullFactorial {
                factors: 3,
                levels: 2
            },
            &c
        ),
        Err(ScientificComputationError::Cancelled)
    ));
}

#[test]
fn uniform_search_is_reproducible_latin_and_retains_the_best_candidate() {
    let c = control();
    let spec = |candidates| Spec::Uniform {
        factors: 4,
        runs: 24,
        candidates,
        seed: 15,
    };
    let initial = generate_design(spec(1), &c).unwrap();
    let best = generate_design(spec(16), &c).unwrap();
    assert!(best.summary.centered_l2_discrepancy <= initial.summary.centered_l2_discrepancy);
    assert_eq!(best.rows, generate_design(spec(16), &c).unwrap().rows);
    for j in 1..=4 {
        let mut values: Vec<_> = best.rows.iter().map(|r| r[j] as usize).collect();
        values.sort_unstable();
        assert_eq!(values, (1..=24).collect::<Vec<_>>());
    }
    let one = generate_design(
        Spec::Uniform {
            factors: 1,
            runs: 640,
            candidates: 1,
            seed: 0,
        },
        &c,
    )
    .unwrap();
    assert_eq!(one.rows.len(), 640);
    assert!(
        (one.summary.centered_l2_discrepancy.unwrap() - 1. / (12. * 640_f64.powi(2))).abs() < 1e-11
    );
}
