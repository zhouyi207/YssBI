use super::*;
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    let id = "yssbi.statistics.sem.path";
    append_node(
        fragment,
        id,
        "Path analysis (observed recursive linear models)",
        "路径分析（观测变量递归线性模型）",
        vec![
            bounded_user_data_input("variables", "Variable (x1, x2, …)", series_type()?, 2, None)?,
            data_output("result", "Path equations", report_type()?)?,
            fixed_numeric_table(
                "effects",
                "Direct, indirect and total effects",
                &[
                    "source",
                    "target",
                    "direct",
                    "indirect",
                    "total",
                    "standardized_total",
                ],
            )?,
            fixed_numeric_table(
                "observations",
                "Per-equation fitted observations",
                &["variable", "observation", "response", "fitted", "residual"],
            )?,
        ],
        vec![parameter(
            "equations",
            concrete("core.text")?,
            ParameterEditorSpec::Text { multiline: true },
            DataValue::String("x2 ~ x1; x3 ~ x1 + x2".into()),
            vec![ParameterConstraint::Required],
        )?],
    )
}
