use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};

#[derive(Clone, Copy)]
enum Method {
    Adf,
    Var,
    LagOrder,
    Vec,
    Rank,
}
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    use Method::*;
    for (id, method, params, outputs) in [
        ("adf.test", Adf, &["lags", "regression"][..], 2),
        ("var.fit", Var, &["lags"][..], 1),
        ("var.lag_order", LagOrder, &["max_lags"][..], 1),
        ("vec.fit", Vec, &["rank", "lags", "trend"][..], 1),
        ("vec.rank_test", Rank, &["max_lags", "trend"][..], 1),
    ] {
        install(
            builder,
            &format!("yssbi.statistics.{id}"),
            if matches!(method, Adf) {
                vec![Input::fixed("series")]
            } else {
                vec![Input::repeated("variables", 2..=usize::MAX)]
            },
            params,
            outputs,
            move |inv| run(method, inv),
        );
    }
    install(
        builder,
        "yssbi.statistics.var.summary",
        vec![Input::fixed("model")],
        &[
            "model_summary",
            "coefficient_table",
            "lag_exclusion",
            "serial_tests",
            "stability",
            "serial_lags",
        ],
        2,
        var_summary,
    );
    install(
        builder,
        "yssbi.statistics.vec.summary",
        vec![Input::fixed("model")],
        &[
            "model_summary",
            "coefficient_table",
            "cointegration",
            "serial_tests",
            "stability",
            "serial_lags",
        ],
        2,
        vec_summary,
    );
}
fn run(method: Method, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let mut data = columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, 0)?;
    let lags = integer(
        inv,
        if matches!(method, Method::LagOrder | Method::Rank) {
            "max_lags"
        } else {
            "lags"
        },
    )?;
    let n = data[0].len();
    if lags >= n || lags > 1000 {
        return Err(KernelError::InvalidParameter);
    }
    check_fit_workspace(
        n,
        data.len()
            .checked_mul(lags + 2)
            .ok_or(KernelError::BudgetExceeded)?,
        true,
        "GLS",
        inv,
    )?;
    use yss_sci_runtime::time_series as ts;
    let result = match method {
        Method::Adf => ts::augmented_dickey_fuller(&data.remove(0), lags, text(inv, "regression")?),
        Method::Var => ts::var_fit(data, lags),
        Method::LagOrder => ts::var_lag_order(data, lags),
        Method::Vec => ts::vec_fit(data, integer(inv, "rank")?, lags, text(inv, "trend")?),
        Method::Rank => ts::vec_rank_test(data, lags, text(inv, "trend")?),
    }
    .map_err(sci)?;
    let result = value(result, inv)?;
    Ok(if matches!(method, Method::Adf) {
        vec![result.clone(), result]
    } else {
        vec![result]
    })
}

fn var_summary(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let fit: yss_sci_contract::time_series::var::VarFit = decode_model(inv)?;
    let options = yss_sci_contract::time_series::var::VarSummaryOptions {
        model_summary: boolean(inv, "model_summary")?,
        coefficient_table: boolean(inv, "coefficient_table")?,
        lag_exclusion: boolean(inv, "lag_exclusion")?,
        serial_tests: boolean(inv, "serial_tests")?,
        stability: boolean(inv, "stability")?,
        serial_lags: serial_lags(inv)?,
    };
    if options.lag_exclusion || options.serial_tests || options.stability {
        check_fit_workspace(
            fit.statistics.observations,
            fit.design.first().map_or(0, Vec::len) + fit.var_names.len(),
            true,
            "GLS",
            inv,
        )?;
    }
    let report = value(
        yss_sci_runtime::time_series::var_summary(&fit, options).map_err(sci)?,
        inv,
    )?;
    Ok(vec![report.clone(), report])
}
fn vec_summary(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let fit: yss_sci_contract::time_series::vec::VecFit = decode_model(inv)?;
    let options = yss_sci_contract::time_series::vec::VecSummaryOptions {
        model_summary: boolean(inv, "model_summary")?,
        coefficient_table: boolean(inv, "coefficient_table")?,
        cointegration: boolean(inv, "cointegration")?,
        serial_tests: boolean(inv, "serial_tests")?,
        stability: boolean(inv, "stability")?,
        serial_lags: serial_lags(inv)?,
    };
    if options.serial_tests || options.stability {
        check_fit_workspace(
            fit.statistics.observations,
            fit.design.first().map_or(0, Vec::len) + fit.var_names.len(),
            true,
            "GLS",
            inv,
        )?;
    }
    let report = value(
        yss_sci_runtime::time_series::vec_summary(&fit, options).map_err(sci)?,
        inv,
    )?;
    Ok(vec![report.clone(), report])
}
fn serial_lags(inv: &KernelInvocation<'_>) -> Result<usize, KernelError> {
    let lags = integer(inv, "serial_lags")?;
    if !(1..=40).contains(&lags) {
        return Err(KernelError::InvalidParameter);
    }
    Ok(lags)
}
