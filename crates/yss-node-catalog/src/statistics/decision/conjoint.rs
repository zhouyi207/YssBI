use super::*;
pub(super) fn implemented(id: &str) -> bool {
    id == "yssbi.statistics.decision.conjoint"
}
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    emit(
        fragment,
        "decision.conjoint",
        "Ratings-based conjoint",
        "联合分析（评分型）",
        vec![
            data_input("ratings", "Profile ratings", series_type()?)?,
            bounded_user_data_input("factors", "Categorical attribute", label_series()?, 1, None)?,
            data_output("result", "Part-worth utilities", report_type()?)?,
            fixed_numeric_table(
                "predictions",
                "Fitted profiles",
                &["observation", "observed", "fitted", "residual"],
            )?,
        ],
        vec![],
    )
}
