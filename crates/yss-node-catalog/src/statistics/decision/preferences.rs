use super::*;
type PreferenceNode = (&'static str, &'static [(&'static str, &'static str)]);
const NODES: &[PreferenceNode] = &[
    ("nps", &[("ratings", "Ratings (0–10)")]),
    (
        "kano",
        &[
            ("functional", "Functional answer (1–5)"),
            ("dysfunctional", "Dysfunctional answer (1–5)"),
        ],
    ),
    (
        "rfm",
        &[
            ("recency", "Days since purchase"),
            ("frequency", "Purchase count"),
            ("monetary", "Net spend"),
        ],
    ),
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(method, inputs) in NODES {
        let mut ports = inputs
            .iter()
            .map(|&(key, title)| data_input(key, title, numeric_data_series_type()))
            .collect::<Result<Vec<_>, BuiltinAssemblyError>>()?;
        ports.push(data_output("result", "Summary", report_type()?)?);
        if method == "rfm" {
            ports.push(fixed_numeric_table(
                "scores",
                "Customer scores",
                &[
                    "observation",
                    "recency_score",
                    "frequency_score",
                    "monetary_score",
                    "total",
                ],
            )?);
        }
        emit(fragment, &format!("decision.{method}"), ports, vec![])?;
    }
    Ok(())
}
