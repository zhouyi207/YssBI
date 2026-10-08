use super::*;

#[test]
fn decision_cost_positions_reject_invalid_items_as_parameter_errors() {
    for (id, mut parameters) in [
        (
            "yssbi.statistics.decision.topsis",
            vec![
                ("weight_method", string("equal")),
                ("normalization", string("vector")),
            ],
        ),
        (
            "yssbi.statistics.decision.vikor",
            vec![("majority_weight", number(0.5))],
        ),
    ] {
        parameters.push((
            "cost_criteria",
            RuntimeValue::List(vec![string("first")].into()),
        ));
        let result = run(
            id,
            &[
                ("criteria", series(&[1., 2., 3.])),
                ("criteria", series(&[3., 2., 1.])),
            ],
            &parameters,
            3,
        );
        assert!(
            matches!(result, Err(KernelError::InvalidParameter)),
            "{id}: {result:?}",
        );
    }
}
