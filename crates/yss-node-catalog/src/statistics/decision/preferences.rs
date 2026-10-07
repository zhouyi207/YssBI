use super::*;
type PreferenceNode = (
    &'static str,
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);
const NODES: &[PreferenceNode] = &[
    (
        "nps",
        "Net promoter score",
        "NPS 净推荐值",
        &[("ratings", "Ratings (0–10)")],
    ),
    (
        "kano",
        "KANO requirements",
        "KANO 模型",
        &[
            ("functional", "Functional answer (1–5)"),
            ("dysfunctional", "Dysfunctional answer (1–5)"),
        ],
    ),
    (
        "rfm",
        "RFM customer scoring",
        "RFM 模型",
        &[
            ("recency", "Days since purchase"),
            ("frequency", "Purchase count"),
            ("monetary", "Net spend"),
        ],
    ),
];
pub(super) fn implemented(id: &str) -> bool {
    id.strip_prefix("yssbi.statistics.decision.")
        .is_some_and(|s| NODES.iter().any(|n| n.0 == s))
}
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(method, en, zh, inputs) in NODES {
        let mut ports = inputs
            .iter()
            .map(|&(key, title)| data_input(key, title, series_type()?))
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
        emit(
            fragment,
            &format!("decision.{method}"),
            en,
            zh,
            ports,
            vec![],
        )?;
    }
    Ok(())
}
