//! Aligned process measurements and crossed-study labels to neutral quality computations.
use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::{execution::ScientificExecutionControl as Control, quality::*};
use yss_sci_runtime::quality as sci;
#[derive(Clone, Copy)]
enum Method {
    Chart,
    Capability,
    Gage,
}
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (id, method, inputs, parameters, outputs) in [
        (
            "yssbi.statistics.plot.control_chart",
            Method::Chart,
            vec![Input::fixed("measurements")],
            &["chart_kind"][..],
            3,
        ),
        (
            "yssbi.statistics.quality.process_capability",
            Method::Capability,
            vec![
                Input::fixed("measurements"),
                Input::repeated("subgroups", 0..=1),
            ],
            &["lower_limit", "upper_limit", "target"][..],
            1,
        ),
        (
            "yssbi.statistics.quality.measurement_system",
            Method::Gage,
            vec![
                Input::fixed("measurements"),
                Input::fixed("parts"),
                Input::fixed("operators"),
            ],
            &["include_interaction"][..],
            1,
        ),
    ] {
        install(builder, id, inputs, parameters, outputs, move |inv| {
            execute(method, inv)
        });
    }
}
fn execute(method: Method, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let (data, retained) = materialize(inv)?;
    let n = data[0].values.len();
    inv.control.check_bytes(
        retained
            .checked_mul(3)
            .and_then(|v| {
                v.checked_add(n.checked_mul(if matches!(method, Method::Chart) {
                    7 * STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES
                        + 6 * (size_of::<RuntimeValue>() * 3 + 16)
                        + 64
                } else {
                    128
                })?)
            })
            .and_then(|v| v.checked_add(131072)),
    )?;
    let measurements = numeric(&data[0], false, inv)?;
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    match method {
        Method::Chart => {
            drop(data);
            let kind = match text(inv, "chart_kind")? {
                "individuals" => ControlChartKind::Individuals,
                "moving_range" => ControlChartKind::MovingRange,
                _ => return Err(KernelError::InvalidParameter),
            };
            let fit =
                sci::control_chart(&measurements, kind, &control).map_err(computation_error)?;
            Ok(vec![
                value(&fit.plot, inv)?,
                value(&fit.summary, inv)?,
                numeric_table(
                    &fit.rows,
                    2,
                    [
                        "observation",
                        "value",
                        "center",
                        "lower",
                        "upper",
                        "outside",
                    ],
                    |r| {
                        [
                            r.observation as f64,
                            r.value,
                            fit.summary.center,
                            fit.summary.lower,
                            fit.summary.upper,
                            if r.outside { 1. } else { 0. },
                        ]
                    },
                    inv,
                )?,
            ])
        }
        Method::Capability => {
            let groups = data
                .get(1)
                .map(|v| categories(v, false, inv).map(|r| r.0))
                .transpose()?;
            drop(data);
            let limits = CapabilityLimits {
                lower: number(inv, "lower_limit")?,
                upper: number(inv, "upper_limit")?,
                target: number(inv, "target")?,
            };
            Ok(vec![value(
                sci::process_capability(&measurements, groups.as_deref(), limits, &control)
                    .map_err(computation_error)?,
                inv,
            )?])
        }
        Method::Gage => {
            let (parts, part_labels) = categories(&data[1], false, inv)?;
            let (operators, operator_labels) = categories(&data[2], false, inv)?;
            drop(data);
            let result = sci::measurement_system(
                &measurements,
                &parts,
                &operators,
                boolean(inv, "include_interaction")?,
                &control,
            )
            .map_err(computation_error)?;
            #[derive(serde::Serialize)]
            struct Report {
                #[serde(flatten)]
                result: GageResult,
                part_labels: Vec<yss_data_contract::TabularScalar>,
                operator_labels: Vec<yss_data_contract::TabularScalar>,
            }
            Ok(vec![value(
                Report {
                    result,
                    part_labels,
                    operator_labels,
                },
                inv,
            )?])
        }
    }
}
