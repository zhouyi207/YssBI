use super::*;
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    emit(
        fragment,
        "workflow.delphi",
        "Delphi round summary",
        "德尔菲法（单轮汇总）",
        vec![
            bounded_user_data_input("criteria", "Rated item", series_type()?, 1, None)?,
            data_output("result", "Concordance", report_type()?)?,
            fixed_numeric_table(
                "items",
                "Item summaries",
                &[
                    "item",
                    "mean",
                    "standard_deviation",
                    "coefficient_of_variation",
                    "q1",
                    "median",
                    "q3",
                    "minimum",
                    "maximum",
                    "full_score_percent",
                ],
            )?,
        ],
        vec![decimal_parameter("full_score", "5")?],
    )
}
