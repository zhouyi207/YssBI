//! 预测节点：基于模型进行预测，输入 pin 根据连接的上游节点（如 OLS 的 Exog 连接）动态生成

use crate::execution::ExecutionEffect;
use crate::execution::context::NodeExecutionContextTrait;
use crate::graph::node::{NodeDefinition, PinResolverContext};
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinDirection, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use ndarray::{Array2, Axis};
use polars::prelude::Series;
use statrs::distribution::{ContinuousCDF, Normal};
use std::sync::Arc;

use super::logit_nodes::LogitModel;
use super::ols_nodes::OLSModel;
use super::probit_nodes::ProbitModel;

const MODEL_ROLE: &str = "prediction_model";
const LOGIT_MODEL_ROLE: &str = "logit_prediction_model";
const PROBIT_MODEL_ROLE: &str = "probit_prediction_model";

fn prediction_input_role(name: &str) -> DataRole {
    DataRole::Custom(format!("pred_exog:{}", name))
}

pub fn register(registry: &NodeRegistry) {
    register_predict(registry);
    register_logit_predict(registry);
    register_probit_predict(registry);
}

fn register_predict(registry: &NodeRegistry) {
    let model_role = PinRole::Data(DataRole::Custom(MODEL_ROLE.to_string()));

    let definition = NodeDefinition::new(
        "Predict",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_documentation(docs::prediction::PREDICT_ZH, docs::prediction::PREDICT_EN)
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::fixed(PinDefinition::data_input(
            "Model",
            DataRole::Custom(MODEL_ROLE.to_string()),
            PinDataTypeDefinition::concrete(DataType::Struct("Model".to_string())),
        )),
        PinSlot::derived_from_input(
            model_role.clone(),
            PinDirection::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(
                vec![DataType::Float64, DataType::Categorical],
            )))),
        ),
        PinSlot::fixed(PinDefinition::data_output(
            "Predicted",
            DataRole::Custom("predicted".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
    ])
    .with_pin_resolver(Arc::new(|ctx: &PinResolverContext| {
        let mut pins = vec![];
        if let Some(schema) = ctx
            .input_schemas
            .get(&PinRole::Data(DataRole::Custom(MODEL_ROLE.to_string())))
        {
            for col in &schema.columns {
                let role = prediction_input_role(&col.name);
                pins.push(
                    PinDefinition::data_input(
                        &col.name,
                        role.clone(),
                        PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(
                            col.data_type.clone(),
                        ))),
                    )
                    .with_dynamic(true),
                );
            }
        }
        Ok(pins)
    }))
    .with_flow_processor(Arc::new(run_predict));
    registry.register(definition);
}

fn run_predict(ctx: &mut dyn NodeExecutionContextTrait) -> Result<ExecutionEffect, String> {
    let model_value =
        ctx.get_input_by_role(&PinRole::Data(DataRole::Custom(MODEL_ROLE.to_string())))?;
    let handle_id = model_value
        .as_handle_id()
        .ok_or("Predict: Model input is not connected or invalid".to_string())?;
    let model = ctx
        .get_handle(&handle_id.to_string())?
        .downcast_ref::<OLSModel>()
        .ok_or("Predict: Model is not an OLSModel".to_string())?
        .clone();

    let mut exog_columns: Vec<Vec<f64>> = Vec::new();
    let mut n: Option<usize> = None;

    for spec in &model.variable_specs {
        match spec {
            super::ols_nodes::VariableSpec::Numeric { name } => {
                let role = prediction_input_role(name);
                let val = ctx.get_input_by_role(&PinRole::Data(role))?;
                let series_id = match &val {
                    DataValue::DataSeries(v) => v.id.clone(),
                    DataValue::Null => {
                        return Err(format!("Predict: Exog '{}' is not connected", name));
                    }
                    _ => return Err(format!("Predict: Exog '{}' is not a DataSeries", name)),
                };
                let series = ctx.get_data_series(&series_id)?;
                let f64_ca = series.f64().map_err(|e| {
                    format!("Predict: Exog '{}' cannot cast to Float64: {}", name, e)
                })?;
                let values: Vec<f64> = f64_ca.into_no_null_iter().collect();
                if let Some(nn) = n {
                    if values.len() != nn {
                        return Err(format!(
                            "Predict: Exog '{}' has {} rows, expected {}",
                            name,
                            values.len(),
                            nn
                        ));
                    }
                } else {
                    n = Some(values.len());
                }
                exog_columns.push(values);
            }
            super::ols_nodes::VariableSpec::Categorical {
                name,
                categories,
                dropped,
                ..
            } => {
                let role = prediction_input_role(name);
                let val = ctx.get_input_by_role(&PinRole::Data(role))?;
                let series_id = match &val {
                    DataValue::DataSeries(v) => v.id.clone(),
                    DataValue::Null => {
                        return Err(format!("Predict: Exog '{}' is not connected", name));
                    }
                    _ => return Err(format!("Predict: Exog '{}' is not a DataSeries", name)),
                };
                let series = ctx.get_data_series(&series_id)?;
                let str_series = series
                    .cast(&polars::prelude::DataType::String)
                    .map_err(|e| {
                        format!("Predict: Exog '{}' cannot cast to String: {}", name, e)
                    })?;
                let str_ca = str_series.str().map_err(|e| {
                    format!("Predict: Exog '{}' cannot cast to String: {}", name, e)
                })?;
                let values: Vec<String> = str_ca
                    .into_iter()
                    .map(|o| o.map(|s| s.to_string()).unwrap_or_default())
                    .collect();

                let categories_to_include: Vec<&String> = if dropped.is_empty() {
                    categories.iter().collect()
                } else {
                    categories.iter().filter(|c| *c != dropped).collect()
                };

                for cat in &categories_to_include {
                    let col: Vec<f64> = values
                        .iter()
                        .map(|v| if v == *cat { 1.0 } else { 0.0 })
                        .collect();
                    if let Some(nn) = n {
                        if col.len() != nn {
                            return Err(format!(
                                "Predict: Exog '{}' has {} rows, expected {}",
                                name,
                                col.len(),
                                nn
                            ));
                        }
                    } else {
                        n = Some(col.len());
                    }
                    exog_columns.push(col);
                }
            }
        }
    }

    let n = n.ok_or("Predict: no exog inputs connected".to_string())?;

    let k = if model.has_constant {
        exog_columns.len() + 1
    } else {
        exog_columns.len()
    };

    let mut exog_raw = Vec::with_capacity(n * k);
    for i in 0..n {
        if model.has_constant {
            exog_raw.push(1.0);
        }
        for col in &exog_columns {
            exog_raw.push(col[i]);
        }
    }

    let exog = Array2::from_shape_vec((n, k), exog_raw)
        .map_err(|e| format!("Predict: failed to build exog matrix: {}", e))?;

    let exog_for_pred = if let Some(ref indices) = model.kept_indices {
        exog.select(Axis(1), indices)
    } else {
        exog.view().to_owned()
    };
    let predicted: Vec<f64> = exog_for_pred
        .rows()
        .into_iter()
        .map(|row| row.iter().zip(model.betas.iter()).map(|(x, b)| x * b).sum())
        .collect();

    let result_series = Series::from_iter(predicted.into_iter()).with_name("predicted".into());
    let result_id = ctx.put_data_series(result_series)?;
    ctx.emit_output_by_role(
        &PinRole::Data(DataRole::Custom("predicted".to_string())),
        DataValue::DataSeries(DataSeriesValue::with_element_type(
            result_id,
            DataType::Float64,
        )),
    )?;

    ctx.log("Predict: completed".to_string());
    Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
}

fn sigmoid(z: f64) -> f64 {
    if z >= 0.0 {
        1.0 / (1.0 + (-z).exp())
    } else {
        let ez = z.exp();
        ez / (1.0 + ez)
    }
}

fn register_logit_predict(registry: &NodeRegistry) {
    let model_role = PinRole::Data(DataRole::Custom(LOGIT_MODEL_ROLE.to_string()));

    let definition = NodeDefinition::new(
        "Logit Predict",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_documentation(
        docs::prediction::LOGIT_PREDICT_ZH,
        docs::prediction::LOGIT_PREDICT_EN,
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::fixed(PinDefinition::data_input(
            "Model",
            DataRole::Custom(LOGIT_MODEL_ROLE.to_string()),
            PinDataTypeDefinition::concrete(DataType::Struct("LogitModel".to_string())),
        )),
        PinSlot::derived_from_input(
            model_role.clone(),
            PinDirection::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(
                vec![DataType::Float64, DataType::Categorical],
            )))),
        ),
        PinSlot::fixed(PinDefinition::data_output(
            "Probability",
            DataRole::Custom("predicted".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
    ])
    .with_pin_resolver(Arc::new(|ctx: &PinResolverContext| {
        let mut pins = vec![];
        if let Some(schema) = ctx.input_schemas.get(&PinRole::Data(DataRole::Custom(
            LOGIT_MODEL_ROLE.to_string(),
        ))) {
            for col in &schema.columns {
                let role = prediction_input_role(&col.name);
                pins.push(
                    PinDefinition::data_input(
                        &col.name,
                        role.clone(),
                        PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(
                            col.data_type.clone(),
                        ))),
                    )
                    .with_dynamic(true),
                );
            }
        }
        Ok(pins)
    }))
    .with_flow_processor(Arc::new(run_logit_predict));
    registry.register(definition);
}

fn run_logit_predict(ctx: &mut dyn NodeExecutionContextTrait) -> Result<ExecutionEffect, String> {
    let model_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom(
        LOGIT_MODEL_ROLE.to_string(),
    )))?;
    let handle_id = model_value
        .as_handle_id()
        .ok_or("Logit Predict: Model input is not connected or invalid".to_string())?;
    let model = ctx
        .get_handle(&handle_id.to_string())?
        .downcast_ref::<LogitModel>()
        .ok_or("Logit Predict: Model is not a LogitModel".to_string())?
        .clone();

    let mut exog_columns: Vec<Vec<f64>> = Vec::new();
    let mut n: Option<usize> = None;

    for spec in &model.variable_specs {
        match spec {
            super::ols_nodes::VariableSpec::Numeric { name } => {
                let role = prediction_input_role(name);
                let val = ctx.get_input_by_role(&PinRole::Data(role))?;
                let series_id = match &val {
                    DataValue::DataSeries(v) => v.id.clone(),
                    DataValue::Null => {
                        return Err(format!("Logit Predict: Exog '{}' is not connected", name));
                    }
                    _ => {
                        return Err(format!(
                            "Logit Predict: Exog '{}' is not a DataSeries",
                            name
                        ));
                    }
                };
                let series = ctx.get_data_series(&series_id)?;
                let f64_ca = series.f64().map_err(|e| {
                    format!(
                        "Logit Predict: Exog '{}' cannot cast to Float64: {}",
                        name, e
                    )
                })?;
                let values: Vec<f64> = f64_ca.into_no_null_iter().collect();
                if let Some(nn) = n {
                    if values.len() != nn {
                        return Err(format!(
                            "Logit Predict: Exog '{}' has {} rows, expected {}",
                            name,
                            values.len(),
                            nn
                        ));
                    }
                } else {
                    n = Some(values.len());
                }
                exog_columns.push(values);
            }
            super::ols_nodes::VariableSpec::Categorical {
                name,
                categories,
                dropped,
                ..
            } => {
                let role = prediction_input_role(name);
                let val = ctx.get_input_by_role(&PinRole::Data(role))?;
                let series_id = match &val {
                    DataValue::DataSeries(v) => v.id.clone(),
                    DataValue::Null => {
                        return Err(format!("Logit Predict: Exog '{}' is not connected", name));
                    }
                    _ => {
                        return Err(format!(
                            "Logit Predict: Exog '{}' is not a DataSeries",
                            name
                        ));
                    }
                };
                let series = ctx.get_data_series(&series_id)?;
                let str_series = series
                    .cast(&polars::prelude::DataType::String)
                    .map_err(|e| {
                        format!(
                            "Logit Predict: Exog '{}' cannot cast to String: {}",
                            name, e
                        )
                    })?;
                let str_ca = str_series.str().map_err(|e| {
                    format!(
                        "Logit Predict: Exog '{}' cannot cast to String: {}",
                        name, e
                    )
                })?;
                let values: Vec<String> = str_ca
                    .into_iter()
                    .map(|o| o.map(|s| s.to_string()).unwrap_or_default())
                    .collect();

                let categories_to_include: Vec<&String> = if dropped.is_empty() {
                    categories.iter().collect()
                } else {
                    categories.iter().filter(|c| *c != dropped).collect()
                };

                for cat in &categories_to_include {
                    let col: Vec<f64> = values
                        .iter()
                        .map(|v| if v == *cat { 1.0 } else { 0.0 })
                        .collect();
                    if let Some(nn) = n {
                        if col.len() != nn {
                            return Err(format!(
                                "Logit Predict: Exog '{}' has {} rows, expected {}",
                                name,
                                col.len(),
                                nn
                            ));
                        }
                    } else {
                        n = Some(col.len());
                    }
                    exog_columns.push(col);
                }
            }
        }
    }

    let n = n.ok_or("Logit Predict: no exog inputs connected".to_string())?;

    let k = if model.has_constant {
        exog_columns.len() + 1
    } else {
        exog_columns.len()
    };

    let mut exog_raw = Vec::with_capacity(n * k);
    for i in 0..n {
        if model.has_constant {
            exog_raw.push(1.0);
        }
        for col in &exog_columns {
            exog_raw.push(col[i]);
        }
    }

    let exog = Array2::from_shape_vec((n, k), exog_raw)
        .map_err(|e| format!("Logit Predict: failed to build exog matrix: {}", e))?;

    let predicted: Vec<f64> = exog
        .rows()
        .into_iter()
        .map(|row| {
            let eta: f64 = row.iter().zip(model.betas.iter()).map(|(x, b)| x * b).sum();
            sigmoid(eta)
        })
        .collect();

    let result_series = Series::from_iter(predicted.into_iter()).with_name("probability".into());
    let result_id = ctx.put_data_series(result_series)?;
    ctx.emit_output_by_role(
        &PinRole::Data(DataRole::Custom("predicted".to_string())),
        DataValue::DataSeries(DataSeriesValue::with_element_type(
            result_id,
            DataType::Float64,
        )),
    )?;

    ctx.log("Logit Predict: completed".to_string());
    Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
}

fn register_probit_predict(registry: &NodeRegistry) {
    let model_role = PinRole::Data(DataRole::Custom(PROBIT_MODEL_ROLE.to_string()));

    let definition = NodeDefinition::new(
        "Probit Predict",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_documentation(
        docs::prediction::PROBIT_PREDICT_ZH,
        docs::prediction::PROBIT_PREDICT_EN,
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::fixed(PinDefinition::data_input(
            "Model",
            DataRole::Custom(PROBIT_MODEL_ROLE.to_string()),
            PinDataTypeDefinition::concrete(DataType::Struct("ProbitModel".to_string())),
        )),
        PinSlot::derived_from_input(
            model_role.clone(),
            PinDirection::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(
                vec![DataType::Float64, DataType::Categorical],
            )))),
        ),
        PinSlot::fixed(PinDefinition::data_output(
            "Probability",
            DataRole::Custom("predicted".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
    ])
    .with_pin_resolver(Arc::new(|ctx: &PinResolverContext| {
        let mut pins = vec![];
        if let Some(schema) = ctx.input_schemas.get(&PinRole::Data(DataRole::Custom(
            PROBIT_MODEL_ROLE.to_string(),
        ))) {
            for col in &schema.columns {
                let role = prediction_input_role(&col.name);
                pins.push(
                    PinDefinition::data_input(
                        &col.name,
                        role.clone(),
                        PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(
                            col.data_type.clone(),
                        ))),
                    )
                    .with_dynamic(true),
                );
            }
        }
        Ok(pins)
    }))
    .with_flow_processor(Arc::new(run_probit_predict));
    registry.register(definition);
}

fn run_probit_predict(ctx: &mut dyn NodeExecutionContextTrait) -> Result<ExecutionEffect, String> {
    let model_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom(
        PROBIT_MODEL_ROLE.to_string(),
    )))?;
    let handle_id = model_value
        .as_handle_id()
        .ok_or("Probit Predict: Model input is not connected or invalid".to_string())?;
    let model = ctx
        .get_handle(&handle_id.to_string())?
        .downcast_ref::<ProbitModel>()
        .ok_or("Probit Predict: Model is not a ProbitModel".to_string())?
        .clone();

    let mut exog_columns: Vec<Vec<f64>> = Vec::new();
    let mut n: Option<usize> = None;

    for spec in &model.variable_specs {
        match spec {
            super::ols_nodes::VariableSpec::Numeric { name } => {
                let role = prediction_input_role(name);
                let val = ctx.get_input_by_role(&PinRole::Data(role))?;
                let series_id = match &val {
                    DataValue::DataSeries(v) => v.id.clone(),
                    DataValue::Null => {
                        return Err(format!("Probit Predict: Exog '{}' is not connected", name));
                    }
                    _ => {
                        return Err(format!(
                            "Probit Predict: Exog '{}' is not a DataSeries",
                            name
                        ));
                    }
                };
                let series = ctx.get_data_series(&series_id)?;
                let f64_ca = series.f64().map_err(|e| {
                    format!(
                        "Probit Predict: Exog '{}' cannot cast to Float64: {}",
                        name, e
                    )
                })?;
                let values: Vec<f64> = f64_ca.into_no_null_iter().collect();
                if let Some(nn) = n {
                    if values.len() != nn {
                        return Err(format!(
                            "Probit Predict: Exog '{}' has {} rows, expected {}",
                            name,
                            values.len(),
                            nn
                        ));
                    }
                } else {
                    n = Some(values.len());
                }
                exog_columns.push(values);
            }
            super::ols_nodes::VariableSpec::Categorical {
                name,
                categories,
                dropped,
                ..
            } => {
                let role = prediction_input_role(name);
                let val = ctx.get_input_by_role(&PinRole::Data(role))?;
                let series_id = match &val {
                    DataValue::DataSeries(v) => v.id.clone(),
                    DataValue::Null => {
                        return Err(format!("Probit Predict: Exog '{}' is not connected", name));
                    }
                    _ => {
                        return Err(format!(
                            "Probit Predict: Exog '{}' is not a DataSeries",
                            name
                        ));
                    }
                };
                let series = ctx.get_data_series(&series_id)?;
                let str_series = series
                    .cast(&polars::prelude::DataType::String)
                    .map_err(|e| {
                        format!(
                            "Probit Predict: Exog '{}' cannot cast to String: {}",
                            name, e
                        )
                    })?;
                let str_ca = str_series.str().map_err(|e| {
                    format!(
                        "Probit Predict: Exog '{}' cannot cast to String: {}",
                        name, e
                    )
                })?;
                let values: Vec<String> = str_ca
                    .into_iter()
                    .map(|o| o.map(|s| s.to_string()).unwrap_or_default())
                    .collect();

                let categories_to_include: Vec<&String> = if dropped.is_empty() {
                    categories.iter().collect()
                } else {
                    categories.iter().filter(|c| *c != dropped).collect()
                };

                for cat in &categories_to_include {
                    let col: Vec<f64> = values
                        .iter()
                        .map(|v| if v == *cat { 1.0 } else { 0.0 })
                        .collect();
                    if let Some(nn) = n {
                        if col.len() != nn {
                            return Err(format!(
                                "Probit Predict: Exog '{}' has {} rows, expected {}",
                                name,
                                col.len(),
                                nn
                            ));
                        }
                    } else {
                        n = Some(col.len());
                    }
                    exog_columns.push(col);
                }
            }
        }
    }

    let n = n.ok_or("Probit Predict: no exog inputs connected".to_string())?;

    let k = if model.has_constant {
        exog_columns.len() + 1
    } else {
        exog_columns.len()
    };

    let mut exog_raw = Vec::with_capacity(n * k);
    for i in 0..n {
        if model.has_constant {
            exog_raw.push(1.0);
        }
        for col in &exog_columns {
            exog_raw.push(col[i]);
        }
    }

    let exog = Array2::from_shape_vec((n, k), exog_raw)
        .map_err(|e| format!("Probit Predict: failed to build exog matrix: {}", e))?;

    let normal = Normal::new(0.0, 1.0).map_err(|e| format!("Probit Predict: Normal: {}", e))?;

    let predicted: Vec<f64> = exog
        .rows()
        .into_iter()
        .map(|row| {
            let eta: f64 = row.iter().zip(model.betas.iter()).map(|(x, b)| x * b).sum();
            normal.cdf(eta)
        })
        .collect();

    let result_series = Series::from_iter(predicted.into_iter()).with_name("probability".into());
    let result_id = ctx.put_data_series(result_series)?;
    ctx.emit_output_by_role(
        &PinRole::Data(DataRole::Custom("predicted".to_string())),
        DataValue::DataSeries(DataSeriesValue::with_element_type(
            result_id,
            DataType::Float64,
        )),
    )?;

    ctx.log("Probit Predict: completed".to_string());
    Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
}
