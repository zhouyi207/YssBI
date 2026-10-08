use super::*;

#[test]
fn moderated_mediation_rejects_invalid_stage_as_parameter_error() {
    let inputs = ["y", "x", "mediator", "moderator"].map(|key| (key, series(&[1., 2., 3., 4.])));
    let result = run(
        "yssbi.statistics.workflow.moderated_mediation",
        &inputs,
        &[
            ("stage", string("unknown")),
            ("probe_sd", number(1.)),
            ("replications", int(10)),
            ("seed", int(7)),
        ],
        2,
    );
    assert!(
        matches!(result, Err(KernelError::InvalidParameter)),
        "{result:?}"
    );
}
