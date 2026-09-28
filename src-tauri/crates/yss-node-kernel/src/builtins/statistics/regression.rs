use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_data_contract::TabularScalar;
use yss_sci_contract::regression::{
    discrete::BinaryOptions,
    fit::BinaryRegressionLink,
    prais::{PraisConfig, PraisTransform},
};

#[derive(Clone, Copy)]
enum Method {
    Binary(BinaryRegressionLink),
    Prais,
}

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (name, method) in [
        ("logit", Method::Binary(BinaryRegressionLink::Logit)),
        ("probit", Method::Binary(BinaryRegressionLink::Probit)),
        ("prais", Method::Prais),
    ] {
        let mut parameters = vec!["constant", "max_iterations", "tolerance"];
        if matches!(method, Method::Prais) {
            parameters.push("transform");
        }
        install(
            builder,
            &format!("yssbi.statistics.{name}.fit"),
            vec![
                Input::fixed("response"),
                Input::repeated("predictors", 1..=usize::MAX),
            ],
            &parameters,
            3,
            move |inv| fit(method, inv),
        );
        install(
            builder,
            &format!("yssbi.statistics.{name}.summary"),
            vec![Input::fixed("model")],
            &[],
            2,
            summary,
        );
        if let Method::Binary(link) = method {
            install(
                builder,
                &format!("yssbi.statistics.{name}.predict"),
                vec![
                    Input::fixed("model"),
                    Input::repeated("predictors", 1..=usize::MAX),
                ],
                &[],
                1,
                move |inv| predict(link, inv),
            );
        }
    }
}

fn fit(method: Method, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let mut data = columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, 0)?;
    let response = data.remove(0);
    let n = response.len();
    let constant = boolean(inv, "constant")?;
    let max_iterations = integer(inv, "max_iterations")?;
    let tolerance = number(inv, "tolerance")?;
    if !(1..=10000).contains(&max_iterations) || tolerance <= 0.0 {
        return Err(KernelError::InvalidParameter);
    }
    check_fit_workspace(n, data.len(), constant, "OLS", inv)?;
    let fit = match method {
        Method::Binary(link) => yss_sci_runtime::regression::discrete::fit_binary(
            link,
            response,
            &data,
            BinaryOptions {
                constant,
                max_iterations,
                tolerance,
            },
            metadata(n),
        ),
        Method::Prais => yss_sci_runtime::regression::linear::prais::fit_prais(
            response,
            &data,
            PraisConfig {
                constant,
                max_iter: max_iterations,
                tol: tolerance,
                transform: match text(inv, "transform")? {
                    "prais_winsten" => PraisTransform::PraisWinsten,
                    "cochrane_orcutt" => PraisTransform::CochraneOrcutt,
                    _ => return Err(KernelError::InvalidParameter),
                },
                ..Default::default()
            },
            metadata(n),
        ),
    }
    .map_err(sci)?;
    inv.check_control()?;
    let model = value(fit, inv)?;
    Ok(vec![
        model.clone(),
        field(&model, "fitted")?.clone(),
        field(&model, "residuals")?.clone(),
    ])
}

fn summary(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let fit = decode_model(inv)?;
    let report = yss_sci_runtime::regression::report::regression_report(&fit).map_err(sci)?;
    let report = value(report, inv)?;
    Ok(vec![report.clone(), report])
}

fn predict(
    link: BinaryRegressionLink,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let model = inv.inputs.first().ok_or(KernelError::InvalidNumericInput)?;
    let constant = match field(model, "constant")? {
        RuntimeValue::Scalar(TabularScalar::Bool(v)) => *v,
        _ => return Err(KernelError::InvalidNumericInput),
    };
    let expected = match link {
        BinaryRegressionLink::Logit => "logit",
        BinaryRegressionLink::Probit => "probit",
    };
    if !matches!(field(model, "family")?, RuntimeValue::Scalar(TabularScalar::String(v)) if v.as_ref() == expected)
    {
        return Err(KernelError::InvalidNumericInput);
    }
    let coefficients = columns(&[field(model, "coefficients")?], inv, 0)?.remove(0);
    let predictors = columns(
        &group(inv, "predictors"),
        inv,
        coefficients.len().saturating_mul(8),
    )?;
    check_fit_workspace(predictors[0].len(), predictors.len(), constant, "OLS", inv)?;
    let prediction = yss_sci_runtime::regression::discrete::predict_binary(
        link,
        &coefficients,
        &predictors,
        constant,
    )
    .map_err(sci)?;
    Ok(vec![numeric_list(&prediction, inv)?])
}
