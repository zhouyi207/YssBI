use super::*;
use yss_sci_contract::regression::models::{GlmFamily, IterationOptions};
const METHODS: &[(&str, Option<GlmFamily>)] = &[
    ("mean_proportion", None),
    ("stratified", None),
    ("clustered", None),
    ("linear_regression", Some(GlmFamily::Gaussian)),
    ("logistic", Some(GlmFamily::Binomial)),
    ("poisson", Some(GlmFamily::Poisson)),
];
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for &(method, family) in METHODS {
        let mut inputs = vec![Input::fixed("response"), Input::fixed("weights")];
        for role in ["strata", "clusters"] {
            let required = (method == "stratified" && role == "strata")
                || (method == "clustered" && role == "clusters");
            inputs.push(if required {
                Input::fixed(role)
            } else {
                Input::repeated(role, 0..=1)
            });
        }
        let mut params = vec!["lonely_psu"];
        if family.is_some() {
            inputs.push(Input::repeated("predictors", 0..=usize::MAX));
            params.extend(["constant", "max_iterations", "tolerance"]);
        } else {
            params.push("statistic");
        }
        install(
            builder,
            &format!("yssbi.statistics.survey.{method}"),
            inputs,
            &params,
            if family.is_some() { 2 } else { 1 },
            move |inv| estimate(inv, family),
        );
    }
}
fn estimate(
    inv: &KernelInvocation<'_>,
    family: Option<GlmFamily>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let (data, retained) = materialize(inv)?;
    let n = data[0].values.len();
    let k = group(inv, "predictors")
        .len()
        .checked_add(1)
        .ok_or(KernelError::BudgetExceeded)?;
    inv.control.check_bytes((|| {
        n.checked_mul(k)?
            .checked_mul(512)?
            .checked_add(k.checked_mul(k)?.checked_mul(2048)?)?
            .checked_add(n.checked_mul(size_of::<RuntimeValue>() * 40)?)?
            .checked_add(retained.checked_mul(3)?)?
            .checked_add(65536)
    })())?;
    let response = numeric(&data[0], true, inv)?;
    let weights = numeric(&data[1], false, inv)?;
    let role = |name: &str| -> Result<Option<Vec<usize>>, KernelError> {
        inv.input_keys
            .iter()
            .position(|k| *k == name)
            .map(|j| categories(&data[j], false, inv).map(|v| v.0))
            .transpose()
    };
    let strata = role("strata")?;
    let clusters = role("clusters")?;
    let design = SurveyDesign {
        weights: &weights,
        strata: strata.as_deref(),
        clusters: clusters.as_deref(),
        lonely_psu: match text(inv, "lonely_psu")? {
            "fail" => LonelyPsu::Fail,
            "certainty" => LonelyPsu::Certainty,
            _ => return Err(KernelError::InvalidParameter),
        },
    };
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    if let Some(family) = family {
        let predictors = inv
            .input_keys
            .iter()
            .enumerate()
            .filter(|(_, key)| **key == "predictors")
            .map(|(i, _)| numeric(&data[i], false, inv))
            .collect::<Result<Vec<_>, _>>()?;
        let result = yss_sci_runtime::survey::regression(
            &response,
            &predictors,
            design,
            SurveyRegressionOptions {
                family,
                constant: boolean(inv, "constant")?,
                iteration: IterationOptions {
                    max_iterations: integer(inv, "max_iterations")?,
                    tolerance: number(inv, "tolerance")?,
                },
            },
            &control,
        )
        .map_err(computation_error)?;
        let labels = group(inv, "predictors")
            .iter()
            .enumerate()
            .map(|(j, v)| input_label(v, format!("x{}", j + 1)))
            .collect();
        regression_outputs(inv, &response, &result.model, labels, &result.diagnostics)
    } else {
        let proportion = match text(inv, "statistic")? {
            "mean" => false,
            "proportion" => true,
            _ => return Err(KernelError::InvalidParameter),
        };
        Ok(vec![value(
            yss_sci_runtime::survey::mean(&response, design, proportion, &control)
                .map_err(computation_error)?,
            inv,
        )?])
    }
}
