//! Prais-Winsten and Cochrane-Orcutt regression nodes
//!
//! Stata: prais y x1 x2 [, corc]

use crate::execution::ExecutionEffect;
use crate::execution::context::NodeExecutionContextTrait;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{CategoricalRole, DataSeriesValue, DataType, DataValue};
use ndarray::{Array1, Array2};
use polars::prelude::{Column, DataFrame, Series};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yss_sci::regression::collinearity;
use yss_sci::regression::diagnostics;
use yss_sci::regression::linear_model::{Prais, PraisConfig, PraisTransform};

use super::info_nodes::{
    Coefficient, DiagnosticInfo, ModelBasicInfo, OLSResult, OmitInfo, OmittedVariable, PraisInfo,
    VifEntry, compute_aic_bic,
};
use super::ols_nodes::VariableSpec;

// ======================== 结构体 ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PraisConfigure {
    pub constant: bool,
    /// "prais" (Prais-Winsten) or "corc" (Cochrane-Orcutt)
    pub transform: String,
}

impl Default for PraisConfigure {
    fn default() -> Self {
        Self {
            constant: true,
            transform: "prais".to_string(),
        }
    }
}

/// Prais 模型（用于 predict，与 OLSModel 兼容但含 rho）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PraisModel {
    pub betas: Vec<f64>,
    pub has_constant: bool,
    pub variable_specs: Vec<VariableSpec>,
    pub rho: f64,
}

struct PraisFitResult {
    ols_result: OLSResult,
    prais_model: PraisModel,
}

// ======================== 共享辅助函数 ========================

fn prais_input_slots() -> Vec<PinSlot> {
    let exog_type = DataType::DataSeries(Box::new(DataType::one_of(vec![
        DataType::Float64,
        DataType::Categorical,
    ])));

    vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::fixed(PinDefinition::data_input(
            "Y",
            DataRole::Custom("y".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::repeatable(
            PinDefinition::data_input(
                "",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(exog_type),
            ),
            "X",
            1,
            None,
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Time",
                DataRole::Custom("time".to_string()),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(
                    vec![DataType::Date, DataType::Int64],
                )))),
            )
            .with_optional(true),
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Config",
                DataRole::Custom("prais_config".to_string()),
                PinDataTypeDefinition::concrete(DataType::Struct("PraisConfigure".to_string())),
            )
            .with_optional(true),
        ),
    ]
}

fn run_prais_regression(ctx: &mut dyn NodeExecutionContextTrait) -> Result<PraisFitResult, String> {
    let endog_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("y".to_string())))?;
    let endog_id = match &endog_value {
        DataValue::DataSeries(v) => v.id.clone(),
        other => {
            let got = match other {
                DataValue::Null => "Null (unconnected or upstream not executed)",
                DataValue::Struct { type_key, .. } => {
                    return Err(format!(
                        "Prais: Y input is not a DataSeries (got Struct<{}>).",
                        type_key
                    ));
                }
                _ => "unexpected type",
            };
            return Err(format!("Prais: Y must be a DataSeries (got {}).", got));
        }
    };
    let endog_series = ctx.get_series(&endog_id)?;
    let endog_name = {
        let raw = endog_series.name().to_string();
        if raw.is_empty() { "y".to_string() } else { raw }
    };
    let endog_f64 = endog_series
        .cast(&polars::prelude::DataType::Float64)
        .map_err(|e| format!("Prais: cannot cast Y to Float64: {}", e))?;

    let config =
        match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("prais_config".to_string()))) {
            Ok(v) => match v.as_handle_id() {
                Some(id) => {
                    let h = ctx.get_handle(&id.to_string())?;
                    h.downcast_ref::<PraisConfigure>()
                        .ok_or("Prais: config is not PraisConfigure")?
                        .clone()
                }
                None => PraisConfigure::default(),
            },
            Err(_) => PraisConfigure::default(),
        };

    let has_constant = config.constant;
    let transform = if config.transform.eq_ignore_ascii_case("corc") {
        PraisTransform::CochraneOrcutt
    } else {
        PraisTransform::PraisWinsten
    };

    let exog_values = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Inputs(0)))?;
    if exog_values.is_empty() {
        return Err("Prais: at least one X required".to_string());
    }

    let n_raw = endog_f64.len();
    let mut df_cols: Vec<Column> = vec![Column::from(endog_f64.with_name("__endog__".into()))];

    let mut exog_meta: Vec<(String, bool, DataSeriesValue)> = Vec::new();
    for (i, val) in exog_values.iter().enumerate() {
        let dsv = match val {
            DataValue::DataSeries(v) => v.clone(),
            _ => return Err(format!("Prais: X {} is not DataSeries", i)),
        };
        let s = ctx.get_series(&dsv.id)?;
        let name = {
            let r = s.name().to_string();
            if r.is_empty() {
                format!("x{}", i + 1)
            } else {
                r
            }
        };
        if s.len() != n_raw {
            return Err(format!(
                "Prais: X '{}' has {} obs, expected {}",
                name,
                s.len(),
                n_raw
            ));
        }
        let is_cat = matches!(
            s.dtype(),
            polars::prelude::DataType::Categorical(_, _) | polars::prelude::DataType::Enum(_, _)
        );
        let col_s = if is_cat {
            s.cast(&polars::prelude::DataType::String)
                .map_err(|e| format!("Prais: {}", e))?
        } else {
            s.cast(&polars::prelude::DataType::Float64)
                .map_err(|e| format!("Prais: {}", e))?
        };
        df_cols.push(Column::from(col_s.with_name(name.as_str().into())));
        exog_meta.push((name, is_cat, dsv));
    }

    let df = DataFrame::new(n_raw, df_cols)
        .map_err(|e| format!("Prais: {}", e))?
        .drop_nulls::<&str>(None)
        .map_err(|e| format!("Prais: drop_nulls: {}", e))?;
    let n = df.height();
    if n < 3 {
        return Err("Prais: need at least 3 observations after dropping nulls".to_string());
    }

    let endog = Array1::from(
        df.column("__endog__")
            .map_err(|e| format!("Prais: {}", e))?
            .f64()
            .map_err(|e| format!("Prais: {}", e))?
            .into_no_null_iter()
            .collect::<Vec<f64>>(),
    );

    let mut exog_columns: Vec<Vec<f64>> = Vec::new();
    let mut col_labels: Vec<(String, Option<String>)> = Vec::new();
    let mut variable_specs: Vec<VariableSpec> = Vec::new();

    for (name, is_cat, dsv) in exog_meta {
        let col = df.column(&name).map_err(|e| format!("Prais: {}", e))?;
        if is_cat {
            let str_ca = col.str().map_err(|e| format!("Prais: {}", e))?;
            let vals: Vec<String> = str_ca
                .into_no_null_iter()
                .map(|s: &str| s.to_string())
                .collect();
            let mut uniq: Vec<String> = Vec::new();
            for v in &vals {
                if !uniq.contains(v) {
                    uniq.push(v.clone());
                }
            }
            if uniq.len() < 2 {
                return Err(format!(
                    "Prais: categorical '{}' needs ≥2 unique values",
                    name
                ));
            }
            let dummy_info = dsv.dummy_info.as_ref();
            let role = dummy_info
                .map(|d| d.role.clone())
                .unwrap_or(CategoricalRole::General);
            let drop_cat = if let Some(di) = dummy_info {
                di.drop_category
                    .as_ref()
                    .cloned()
                    .filter(|c| uniq.contains(c))
                    .unwrap_or_else(|| uniq[0].clone())
            } else if has_constant {
                uniq[0].clone()
            } else {
                String::new()
            };
            let to_include: Vec<&String> = if drop_cat.is_empty() {
                uniq.iter().collect()
            } else {
                uniq.iter().filter(|c| **c != drop_cat).collect()
            };
            for cat in &to_include {
                let vec: Vec<f64> = vals
                    .iter()
                    .map(|v| if v == *cat { 1.0 } else { 0.0 })
                    .collect();
                exog_columns.push(vec);
                col_labels.push((name.clone(), Some((*cat).clone())));
            }
            variable_specs.push(VariableSpec::Categorical {
                name,
                categories: uniq,
                dropped: drop_cat,
                role,
            });
        } else {
            let vec: Vec<f64> = col
                .f64()
                .map_err(|e| format!("Prais: {}", e))?
                .into_no_null_iter()
                .collect();
            exog_columns.push(vec);
            col_labels.push((name.clone(), None));
            variable_specs.push(VariableSpec::Numeric { name });
        }
    }

    let k = if has_constant {
        exog_columns.len() + 1
    } else {
        exog_columns.len()
    };
    let mut exog_raw = Vec::with_capacity(n * k);
    let mut all_labels: Vec<(String, Option<String>)> = Vec::new();
    if has_constant {
        all_labels.push(("const".to_string(), None));
    }
    all_labels.extend(col_labels);

    for i in 0..n {
        if has_constant {
            exog_raw.push(1.0);
        }
        for col in &exog_columns {
            exog_raw.push(col[i]);
        }
    }

    let exog = Array2::from_shape_vec((n, k), exog_raw)
        .map_err(|e| format!("Prais: exog matrix: {}", e))?;

    // ---- Drop strictly collinear columns (continuous > dummy > intercept) ----
    let col_is_dummy: Vec<bool> = all_labels.iter().map(|(_, cat)| cat.is_some()).collect();
    let intercept_col = if has_constant { Some(0) } else { None };
    let (exog_use, omitted_indices) =
        collinearity::drop_collinear_columns(&exog, &col_is_dummy, intercept_col)?;
    let omit_info = if omitted_indices.is_empty() {
        None
    } else {
        let omitted: Vec<OmittedVariable> = omitted_indices
            .iter()
            .filter_map(|&i| all_labels.get(i))
            .map(|(var, cat)| OmittedVariable {
                variable: var.clone(),
                category: cat.clone(),
                reason: "collinearity".to_string(),
            })
            .collect();
        Some(OmitInfo { omitted })
    };
    let all_labels_use: Vec<(String, Option<String>)> = (0..k)
        .filter(|i| !omitted_indices.contains(i))
        .filter_map(|i| all_labels.get(i).cloned())
        .collect();

    let sci_config = PraisConfig {
        constant: has_constant,
        transform,
        rhotype: yss_sci::regression::linear_model::RhoType::Regress,
        max_iter: 100,
        tol: 1e-6,
    };
    let prais = Prais {
        endog: endog.clone(),
        exog: exog_use.clone(),
        config: sci_config,
    };
    let result = prais.fit()?;

    let fitted_values: Vec<f64> = (0..n)
        .map(|i| {
            exog_use
                .row(i)
                .iter()
                .zip(result.betas.iter())
                .map(|(x, b)| x * b)
                .sum()
        })
        .collect();
    let residuals: Vec<f64> = endog
        .iter()
        .zip(fitted_values.iter())
        .map(|(y, yh)| y - yh)
        .collect();

    let num_coeff = result.betas.len();
    let mut coefficients = Vec::with_capacity(num_coeff);
    for i in 0..num_coeff {
        let (var, cat) = all_labels_use
            .get(i)
            .cloned()
            .unwrap_or_else(|| (format!("x{}", i), None));
        coefficients.push(Coefficient {
            variable: var,
            category: cat,
            coef: result.betas[i],
            std_err: result.stds[i],
            t_value: result.tvalues[i],
            p_value: result.pvalues[i],
            ci_lower: result.conf_int_left[i],
            ci_upper: result.conf_int_right[i],
            is_significant: result.pvalues[i] < 0.05,
        });
    }

    let (aic, bic) = compute_aic_bic(
        result.num_observation,
        result.betas.len(),
        result.ss_residual,
    );

    let vif = diagnostics::vif_centered(&exog_use, has_constant)
        .ok()
        .and_then(|entries| {
            let vif_entries: Vec<VifEntry> = entries
                .into_iter()
                .enumerate()
                .filter(|(j, e)| !(has_constant && *j == 0) && !e.vif.is_nan())
                .map(|(j, e)| {
                    let (var_name, cat) = all_labels_use
                        .get(j)
                        .cloned()
                        .unwrap_or_else(|| (format!("x{}", j), None));
                    VifEntry {
                        variable: var_name,
                        category: cat,
                        vif: e.vif,
                        tolerance: e.tolerance,
                    }
                })
                .collect();
            if vif_entries.is_empty() {
                None
            } else {
                Some(vif_entries)
            }
        });

    let method = match result.covariance_type.as_str() {
        s if s.starts_with("Cochrane") => "Cochrane-Orcutt",
        _ => "Prais-Winsten",
    };

    let ols_result = OLSResult {
        title: format!("{} AR(1) Regression Results", method),
        endog_name,
        model_basic_info: ModelBasicInfo {
            model_type: method.to_string(),
            method: "AR(1) regression".to_string(),
            num_observation: result.num_observation,
            r_squared: result.r2,
            adj_r_squared: result.r2_adjusted,
            f_statistic: result.fvalue,
            prob_f_statistic: result.f_p_value,
            wald_chi2: None,
            prob_wald_chi2: None,
            log_likelihood: None,
            lr_chi2: None,
            prob_lr_chi2: None,
            chibar2: None,
            prob_chibar2: None,
            mle_iter_log_lik_const: None,
            mle_iter_log_lik: None,
            df_model: result.df_model,
            df_residual: result.df_residual,
            df_total: result.df_total,
            ss_model: result.ss_model,
            ss_residual: result.ss_residual,
            ss_total: result.ss_total,
            ms_model: result.ms_model,
            ms_residual: result.ms_residual,
            ms_total: result.ms_total,
            covariance_type: result.covariance_type,
            aic,
            bic,
        },
        coefficients,
        diagnostic_info: DiagnosticInfo {
            cond_no: result.cond_no,
            vif,
            bp_tests: None,
            ov_tests: None,
            im_test: None,
            normality_tests: None,
            fitted_values: fitted_values.clone(),
            residuals: residuals.clone(),
            leverage: diagnostics::leverage(&exog_use).unwrap_or_default(),
            residual_scatter: None,
            exog: Some(
                (0..n)
                    .map(|i| exog_use.row(i).iter().cloned().collect())
                    .collect(),
            ),
            timing: None,
            prais_info: Some(PraisInfo {
                rho: result.rho,
                dw_original: result.dw_original,
                dw_transformed: result.dw_transformed,
                iterations: result.iterations,
                iteration_log: result.iteration_log,
            }),
            iv2sls_first_stage: None,
            iv2sls_first_stage_summary: None,
            iv2sls_overid: None,
            iv2sls_overid_dims: None,
            iv2sls_hausman: None,
            iv2sls_endogenous: None,
            ivliml_kappa: None,
            ivliml_overid: None,
            classification_table: None,
            exog_means: None,
            panel_fe_info: None,
            omit_info,
        },
        betas: result.betas.to_vec(),
        cov_beta: (0..result.cov_beta.nrows())
            .map(|i| result.cov_beta.row(i).iter().cloned().collect())
            .collect(),
        cov_beta_nonrobust: None,
    };

    let prais_model = PraisModel {
        betas: result.betas.to_vec(),
        has_constant,
        variable_specs,
        rho: result.rho,
    };

    Ok(PraisFitResult {
        ols_result,
        prais_model,
    })
}

// ======================== 注册 ========================

pub fn register(registry: &NodeRegistry) {
    register_prais_configure(registry);
    register_prais(registry);
    register_prais_summary(registry);
}

fn register_prais_configure(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Prais Configure",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_localized_description(
        "Prais-Winsten / Cochrane-Orcutt 配置",
        "Prais-Winsten / Cochrane-Orcutt configuration",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(
            PinDefinition::data_input(
                "Constant",
                DataRole::Custom("constant".to_string()),
                PinDataTypeDefinition::concrete(DataType::Boolean),
            )
            .with_optional(true),
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Transform",
                DataRole::Custom("transform".to_string()),
                PinDataTypeDefinition::concrete(DataType::String),
            )
            .with_optional(true)
            .with_metadata(true, "dropdown")
            .with_widget_options(vec!["prais".to_string(), "corc".to_string()]),
        ),
        PinSlot::fixed(PinDefinition::data_output(
            "Config",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("PraisConfigure".to_string())),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let constant = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("constant".to_string())))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let transform = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("transform".to_string())))
            .ok()
            .and_then(|v| v.as_string().map(|s| s.to_string()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "prais".to_string());
        let config = PraisConfigure {
            constant,
            transform: transform.to_lowercase(),
        };
        let id = ctx.put_handle(Box::new(config));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("PraisConfigure", id),
        )?;
        Ok(())
    }));
    registry.register(def);
}

fn register_prais(registry: &NodeRegistry) {
    let mut slots = prais_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Model",
        DataRole::Custom("prais_model".to_string()),
        PinDataTypeDefinition::concrete(DataType::Struct("PraisModel".to_string())),
    )));
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Fitted",
        DataRole::Custom("prais_fitted".to_string()),
        PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
    )));
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Residuals",
        DataRole::Custom("prais_residuals".to_string()),
        PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output(
        "Out",
        ExecRole::ExecOut,
    )));

    let def = NodeDefinition::new("Prais", vec!["Data".to_string(), "Statistics".to_string()])
        .with_ui_style("dataframe")
        .with_localized_description(
            "Prais-Winsten / Cochrane-Orcutt AR(1) 回归 — Stata prais",
            "Prais-Winsten / Cochrane-Orcutt AR(1) regression — Stata prais",
        )
        .with_pin_slots(slots)
        .with_flow_processor(Arc::new(|ctx| {
            let fit = run_prais_regression(ctx)?;
            let model_id = ctx.put_handle(Box::new(fit.prais_model));
            ctx.emit_output_by_role(
                &PinRole::Data(DataRole::Custom("prais_model".to_string())),
                DataValue::new_struct("PraisModel", model_id),
            )?;
            let fitted_s =
                Series::from_iter(fit.ols_result.diagnostic_info.fitted_values.into_iter())
                    .with_name("fitted".into());
            let fitted_id = ctx.put_series(fitted_s)?;
            ctx.emit_output_by_role(
                &PinRole::Data(DataRole::Custom("prais_fitted".to_string())),
                DataValue::DataSeries(DataSeriesValue::with_element_type(
                    fitted_id,
                    DataType::Float64,
                )),
            )?;
            let res_s = Series::from_iter(fit.ols_result.diagnostic_info.residuals.into_iter())
                .with_name("residuals".into());
            let res_id = ctx.put_series(res_s)?;
            ctx.emit_output_by_role(
                &PinRole::Data(DataRole::Custom("prais_residuals".to_string())),
                DataValue::DataSeries(DataSeriesValue::with_element_type(
                    res_id,
                    DataType::Float64,
                )),
            )?;
            ctx.log("Prais: regression completed".to_string());
            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));
    registry.register(def);
}

fn register_prais_summary(registry: &NodeRegistry) {
    let mut slots = prais_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Result",
        DataRole::Result,
        PinDataTypeDefinition::concrete(DataType::Struct("OLSResult".to_string())),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output(
        "Out",
        ExecRole::ExecOut,
    )));

    let def = NodeDefinition::new(
        "Prais Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_localized_description(
        "Prais-Winsten / Cochrane-Orcutt — 输出结果并打开 Summary 窗口",
        "Prais-Winsten / Cochrane-Orcutt — outputs results and opens summary window",
    )
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let fit = run_prais_regression(ctx)?;
        let json_data = serde_json::to_string(&fit.ols_result)
            .map_err(|e| format!("Prais Summary: serialize: {}", e))?;
        let result_id = ctx.put_handle(Box::new(fit.ols_result));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSResult", result_id),
        )?;
        ctx.open_window("ols_summary".to_string(), json_data);
        ctx.log("Prais Summary: completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(def);
}
