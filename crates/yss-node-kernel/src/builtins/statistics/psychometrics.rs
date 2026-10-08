//! Questionnaire adapters own alignment, budgets and named table projection.
use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::{
    execution::ScientificExecutionControl as Control, psychometrics::ReliabilityItem,
};
use yss_sci_runtime::psychometrics as sci;
#[derive(Clone, Copy, PartialEq)]
enum Method {
    Reliability,
    Validity,
    Content,
    Items,
}
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (suffix, method, minimum, parameters, outputs) in [
        ("reliability", Method::Reliability, 2, &[][..], 2),
        ("validity", Method::Validity, 2, &[][..], 1),
        ("content_validity", Method::Content, 1, &[][..], 2),
        ("item_analysis", Method::Items, 2, &["tail_fraction"][..], 3),
    ] {
        install(
            builder,
            &format!("yssbi.statistics.psychometrics.{suffix}"),
            vec![Input::repeated("items", minimum..=usize::MAX)],
            parameters,
            outputs,
            move |inv| execute(method, inv),
        );
    }
}
fn named(
    summary: impl serde::Serialize,
    inv: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    #[derive(serde::Serialize)]
    struct Report<T> {
        #[serde(flatten)]
        summary: T,
        item_names: Vec<String>,
    }
    value(
        Report {
            summary,
            item_names: group(inv, "items")
                .iter()
                .enumerate()
                .map(|(j, v)| input_label(v, format!("item{}", j + 1)))
                .collect(),
        },
        inv,
    )
}
fn execute(method: Method, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let (inputs, retained) = materialize(inv)?;
    let n = inputs.first().map_or(0, |c| c.values.len());
    let p = inputs.len();
    inv.control.check_bytes((|| {
        let matrix = if method == Method::Validity {
            n.checked_mul(p)?
                .checked_mul(128)?
                .checked_add(p.checked_mul(p)?.checked_mul(384)?)?
        } else {
            n.checked_mul(p)?.checked_mul(16)?
        };
        retained
            .checked_add(matrix)?
            .checked_add(n.checked_mul(3 * (size_of::<RuntimeValue>() * 3 + 64))?)?
            .checked_add(p.checked_mul(32 * STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?)?
            .checked_add(65536)
    })())?;
    let columns = inputs
        .into_iter()
        .map(|v| numeric(&v, false, inv))
        .collect::<Result<Vec<_>, _>>()?;
    let c = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    match method {
        Method::Validity => Ok(vec![named(
            sci::validity(&columns, &c).map_err(computation_error)?,
            inv,
        )?]),
        Method::Reliability => {
            let fit = sci::reliability(&columns, &c).map_err(computation_error)?;
            Ok(vec![
                named(&fit.summary, inv)?,
                numeric_table(&fit.rows, 1, RELIABILITY_FIELDS, item_values, inv)?,
            ])
        }
        Method::Content => {
            let fit = sci::content_validity(&columns, &c).map_err(computation_error)?;
            Ok(vec![
                named(&fit.summary, inv)?,
                numeric_table(
                    &fit.rows,
                    1,
                    [
                        "item",
                        "relevant_experts",
                        "item_cvi",
                        "chance_agreement",
                        "modified_kappa",
                    ],
                    |r| {
                        [
                            r.item as f64,
                            r.relevant_experts as f64,
                            r.item_cvi,
                            r.chance_agreement,
                            r.modified_kappa,
                        ]
                    },
                    inv,
                )?,
            ])
        }
        Method::Items => {
            let fit = sci::item_analysis(&columns, number(inv, "tail_fraction")?, &c)
                .map_err(computation_error)?;
            Ok(vec![
                named(&fit.summary, inv)?,
                numeric_table(
                    &fit.rows,
                    1,
                    [
                        "item",
                        "mean",
                        "standard_deviation",
                        "corrected_item_total_correlation",
                        "alpha_if_deleted",
                        "low_mean",
                        "high_mean",
                        "t_statistic",
                        "degrees_of_freedom",
                        "p_value",
                    ],
                    |r| {
                        let [a, b, c, d, e] = item_values(&r.reliability);
                        [
                            a,
                            b,
                            c,
                            d,
                            e,
                            r.low_mean,
                            r.high_mean,
                            r.t_statistic,
                            r.degrees_of_freedom,
                            r.p_value,
                        ]
                    },
                    inv,
                )?,
                numeric_table(
                    &fit.scores,
                    2,
                    ["observation", "total", "group"],
                    |r| [r.observation as f64, r.total, r.group as f64],
                    inv,
                )?,
            ])
        }
    }
}
const RELIABILITY_FIELDS: [&str; 5] = [
    "item",
    "mean",
    "standard_deviation",
    "corrected_item_total_correlation",
    "alpha_if_deleted",
];
fn item_values(r: &ReliabilityItem) -> [Option<f64>; 5] {
    [
        Some(r.item as f64),
        Some(r.mean),
        Some(r.standard_deviation),
        r.corrected_item_total_correlation,
        r.alpha_if_deleted,
    ]
}
