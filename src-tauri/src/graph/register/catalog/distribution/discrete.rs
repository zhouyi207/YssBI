//! 离散分布节点：Bernoulli, Binomial, Poisson, Geometric, NegativeBinomial, DiscreteUniform, Hypergeometric

use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use polars::prelude::Series;
use rand::SeedableRng;
use rand::distributions::Distribution;
use rand::rngs::StdRng;
use std::sync::Arc;

fn float_input(
    ctx: &dyn crate::execution::NodeExecutionContextTrait,
    role: DataRole,
) -> Result<f64, String> {
    let v = ctx.get_input_by_role(&PinRole::Data(role))?;
    match v {
        DataValue::Float64(f) => Ok(f),
        DataValue::Float32(f) => Ok(f as f64),
        DataValue::Int64(i) => Ok(i as f64),
        DataValue::Int32(i) => Ok(i as f64),
        _ => Err(format!("Expected numeric, got {:?}", v.value_type())),
    }
}

fn int_input(
    ctx: &dyn crate::execution::NodeExecutionContextTrait,
    role: DataRole,
) -> Result<i64, String> {
    let v = ctx.get_input_by_role(&PinRole::Data(role))?;
    match v {
        DataValue::Int64(i) => Ok(i),
        DataValue::Int32(i) => Ok(i as i64),
        DataValue::Float64(f) => Ok(f as i64),
        DataValue::Float32(f) => Ok(f as i64),
        _ => Err(format!("Expected integer, got {:?}", v.value_type())),
    }
}

fn emit_int_series(
    ctx: &mut dyn crate::execution::NodeExecutionContextTrait,
    values: Vec<i64>,
    name: &str,
) -> Result<(), String> {
    let s = Series::from_iter(values.into_iter()).with_name(name.into());
    let id = ctx.put_data_series(s)?;
    ctx.emit_output_by_role(
        &PinRole::Data(DataRole::Output),
        DataValue::DataSeries(DataSeriesValue::with_element_type(id, DataType::Int64)),
    )?;
    Ok(())
}

fn float_type() -> PinDataTypeDefinition {
    PinDataTypeDefinition::concrete(DataType::Float64)
}

fn int_type() -> PinDataTypeDefinition {
    PinDataTypeDefinition::concrete(DataType::Int64)
}

pub fn register(registry: &NodeRegistry) {
    register_bernoulli(registry);
    register_binomial(registry);
    register_poisson(registry);
    register_geometric(registry);
    register_negative_binomial(registry);
    register_discrete_uniform(registry);
    register_hypergeometric(registry);
}

fn register_bernoulli(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Bernoulli",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Bernoulli(p) 分布抽样，输出 0 或 1",
        "Sample from Bernoulli(p) distribution, outputs 0 or 1",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "P",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(1),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let p = float_input(ctx, DataRole::Inputs(0))?;
        let n = int_input(ctx, DataRole::Inputs(1))?;
        if n < 0 {
            return Err("Bernoulli: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::Bernoulli::new(p)
            .map_err(|e| format!("Bernoulli: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<i64> = (0..n).map(|_| dist.sample(&mut rng) as i64).collect();
        emit_int_series(ctx, values, "bernoulli")
    }));
    registry.register(def);
}

fn register_binomial(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Binomial",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Binomial(n_trials, p) 分布抽样",
        "Sample from Binomial(n_trials, p) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "N Trials",
            DataRole::Inputs(0),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "P",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N Samples",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let n_trials = int_input(ctx, DataRole::Inputs(0))?;
        let p = float_input(ctx, DataRole::Inputs(1))?;
        let n_samples = int_input(ctx, DataRole::Inputs(2))?;
        if n_samples < 0 {
            return Err("Binomial: N Samples must be non-negative".to_string());
        }
        if n_trials < 0 {
            return Err("Binomial: N Trials must be non-negative".to_string());
        }
        let dist = statrs::distribution::Binomial::new(p, n_trials as u64)
            .map_err(|e| format!("Binomial: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<i64> = (0..n_samples)
            .map(|_| dist.sample(&mut rng) as i64)
            .collect();
        emit_int_series(ctx, values, "binomial")
    }));
    registry.register(def);
}

fn register_poisson(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Poisson",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Poisson(λ) 分布抽样",
        "Sample from Poisson(lambda) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Lambda",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(1),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let lambda = float_input(ctx, DataRole::Inputs(0))?;
        let n = int_input(ctx, DataRole::Inputs(1))?;
        if n < 0 {
            return Err("Poisson: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::Poisson::new(lambda)
            .map_err(|e| format!("Poisson: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<i64> = (0..n).map(|_| dist.sample(&mut rng) as i64).collect();
        emit_int_series(ctx, values, "poisson")
    }));
    registry.register(def);
}

fn register_geometric(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Geometric",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Geometric(p) 分布抽样",
        "Sample from Geometric(p) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "P",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(1),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let p = float_input(ctx, DataRole::Inputs(0))?;
        let n = int_input(ctx, DataRole::Inputs(1))?;
        if n < 0 {
            return Err("Geometric: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::Geometric::new(p)
            .map_err(|e| format!("Geometric: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<i64> = (0..n).map(|_| dist.sample(&mut rng) as i64).collect();
        emit_int_series(ctx, values, "geometric")
    }));
    registry.register(def);
}

fn register_negative_binomial(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "NegativeBinomial",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 NegativeBinomial(r, p) 分布抽样",
        "Sample from NegativeBinomial(r, p) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "R",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "P",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let r = float_input(ctx, DataRole::Inputs(0))?;
        let p = float_input(ctx, DataRole::Inputs(1))?;
        let n = int_input(ctx, DataRole::Inputs(2))?;
        if n < 0 {
            return Err("NegativeBinomial: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::NegativeBinomial::new(r, p)
            .map_err(|e| format!("NegativeBinomial: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<i64> = (0..n).map(|_| dist.sample(&mut rng) as i64).collect();
        emit_int_series(ctx, values, "negative_binomial")
    }));
    registry.register(def);
}

fn register_discrete_uniform(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "DiscreteUniform",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 DiscreteUniform(low, high) 分布抽样（含端点）",
        "Sample from DiscreteUniform(low, high) distribution, inclusive",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Low",
            DataRole::Inputs(0),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "High",
            DataRole::Inputs(1),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let low = int_input(ctx, DataRole::Inputs(0))?;
        let high = int_input(ctx, DataRole::Inputs(1))?;
        let n = int_input(ctx, DataRole::Inputs(2))?;
        if n < 0 {
            return Err("DiscreteUniform: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::DiscreteUniform::new(low, high)
            .map_err(|e| format!("DiscreteUniform: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<i64> = (0..n).map(|_| dist.sample(&mut rng) as i64).collect();
        emit_int_series(ctx, values, "discrete_uniform")
    }));
    registry.register(def);
}

fn register_hypergeometric(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Hypergeometric",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Hypergeometric(N, K, n) 分布抽样",
        "Sample from Hypergeometric(N, K, n) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(0),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "K",
            DataRole::Inputs(1),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "n",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N Samples",
            DataRole::Inputs(3),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Int64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let n_pop = int_input(ctx, DataRole::Inputs(0))?;
        let k = int_input(ctx, DataRole::Inputs(1))?;
        let n_draw = int_input(ctx, DataRole::Inputs(2))?;
        let n_samples = int_input(ctx, DataRole::Inputs(3))?;
        if n_samples < 0 {
            return Err("Hypergeometric: N Samples must be non-negative".to_string());
        }
        let dist = statrs::distribution::Hypergeometric::new(n_pop as u64, k as u64, n_draw as u64)
            .map_err(|e| format!("Hypergeometric: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<i64> = (0..n_samples)
            .map(|_| dist.sample(&mut rng) as i64)
            .collect();
        emit_int_series(ctx, values, "hypergeometric")
    }));
    registry.register(def);
}
