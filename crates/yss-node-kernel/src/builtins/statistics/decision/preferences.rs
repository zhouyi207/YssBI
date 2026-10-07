use super::*;
use yss_sci_runtime::decision::preferences;
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (method, ports) in [
        ("nps", &["ratings"][..]),
        ("kano", &["functional", "dysfunctional"][..]),
        ("rfm", &["recency", "frequency", "monetary"][..]),
    ] {
        install(
            builder,
            &format!("yssbi.statistics.decision.{method}"),
            ports.iter().map(|&key| Input::fixed(key)).collect(),
            &[],
            if method == "rfm" { 2 } else { 1 },
            move |inv| execute(method, inv),
        );
    }
}
fn execute(method: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let inputs = inv.inputs.iter().collect::<Vec<_>>();
    let data = super::super::common::columns(&inputs, inv, 0)?;
    let n = data[0].len();
    let row_bytes = if method == "rfm" {
        5 * (size_of::<RuntimeValue>() * 3 + 32)
    } else {
        24
    };
    inv.control
        .check_bytes(n.checked_mul(row_bytes).and_then(|b| b.checked_add(65536)))?;
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    Ok(match method {
        "nps" => vec![value(
            preferences::nps(&data[0], &control).map_err(computation_error)?,
            inv,
        )?],
        "kano" => vec![value(
            preferences::kano(&data[0], &data[1], &control).map_err(computation_error)?,
            inv,
        )?],
        "rfm" => {
            let result = preferences::rfm(&data, &control).map_err(computation_error)?;
            vec![
                value(&result.summary, inv)?,
                numeric_table(
                    &result.rows,
                    1,
                    [
                        "observation",
                        "recency_score",
                        "frequency_score",
                        "monetary_score",
                        "total",
                    ],
                    |r| {
                        [
                            r.observation as f64,
                            r.recency_score as f64,
                            r.frequency_score as f64,
                            r.monetary_score as f64,
                            r.total as f64,
                        ]
                    },
                    inv,
                )?,
            ]
        }
        _ => return Err(KernelError::InvalidParameter),
    })
}
