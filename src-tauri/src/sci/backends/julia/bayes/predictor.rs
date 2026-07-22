use std::collections::BTreeMap;

use crate::sci::api::bayes::{BayesModelSpec, BinaryOp, Expression, MathFunction, UnaryOp};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JuliaPredictorKernel {
    pub predictor_source: String,
    pub likelihood_source: String,
    pub columns: Vec<String>,
}

pub fn compile_predictor(spec: &BayesModelSpec) -> Result<JuliaPredictorKernel, String> {
    let parameters = spec
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.name.as_str(), index + 1))
        .collect::<BTreeMap<_, _>>();
    let mut compiler = PredictorCompiler {
        parameters,
        data_variables: &spec.data_variables,
        columns: Vec::new(),
    };
    let expression = compiler.emit(&spec.predictor)?;

    Ok(JuliaPredictorKernel {
        predictor_source: format!(
            "function (theta, columns, row_index)\n    @inbounds return {expression}\nend\n"
        ),
        likelihood_source: emit_likelihood_kernel(spec, &expression)?,
        columns: compiler.columns,
    })
}

struct PredictorCompiler<'a> {
    parameters: BTreeMap<&'a str, usize>,
    data_variables: &'a BTreeMap<String, String>,
    columns: Vec<String>,
}

impl PredictorCompiler<'_> {
    fn emit(&mut self, expression: &Expression) -> Result<String, String> {
        match expression {
            Expression::Number { value } => emit_number(*value),
            Expression::Parameter { name } => self.emit_parameter(name),
            Expression::DataVariable { name } => {
                let column = self.data_variables.get(name).ok_or_else(|| {
                    format!("Predictor data variable `{name}` has no column binding.")
                })?;
                self.emit_column(column)
            }
            Expression::Column { name } => self.emit_column(name),
            Expression::Unary {
                op: UnaryOp::Neg,
                arg,
            } => Ok(format!("(-{})", self.emit(arg)?)),
            Expression::Binary { op, left, right } => {
                let operator = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Pow => "^",
                };
                Ok(format!(
                    "({} {operator} {})",
                    self.emit(left)?,
                    self.emit(right)?
                ))
            }
            Expression::Call { function, args } => self.emit_call(*function, args),
        }
    }

    fn emit_parameter(&self, name: &str) -> Result<String, String> {
        let index = self
            .parameters
            .get(name)
            .ok_or_else(|| format!("Predictor parameter `{name}` was not declared."))?;
        Ok(format!("theta[{index}]"))
    }

    fn emit_column(&mut self, name: &str) -> Result<String, String> {
        let index = match self.columns.iter().position(|column| column == name) {
            Some(index) => index + 1,
            None => {
                self.columns.push(name.to_string());
                self.columns.len()
            }
        };
        Ok(format!("columns[row_index, {index}]"))
    }

    fn emit_call(&mut self, function: MathFunction, args: &[Expression]) -> Result<String, String> {
        let (name, arity) = match function {
            MathFunction::Exp => ("exp", Arity::Exact(1)),
            MathFunction::Ln => ("log", Arity::Exact(1)),
            MathFunction::Sqrt => ("sqrt", Arity::Exact(1)),
            MathFunction::Abs => ("abs", Arity::Exact(1)),
            MathFunction::Sin => ("sin", Arity::Exact(1)),
            MathFunction::Cos => ("cos", Arity::Exact(1)),
            MathFunction::Min => ("min", Arity::AtLeast(2)),
            MathFunction::Max => ("max", Arity::AtLeast(2)),
        };
        arity.validate(name, args.len())?;
        let arguments = args
            .iter()
            .map(|arg| self.emit(arg))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(format!("{name}({})", arguments.join(", ")))
    }
}

fn emit_likelihood_kernel(spec: &BayesModelSpec, predictor: &str) -> Result<String, String> {
    use crate::sci::api::bayes::LikelihoodSpec;

    let contribution = match &spec.likelihood {
        LikelihoodSpec::Normal { sigma, .. } => {
            let sigma_index = spec
                .parameters
                .iter()
                .position(|parameter| parameter.name == sigma.parameter)
                .ok_or_else(|| {
                    format!(
                        "Likelihood parameter `{}` was not declared.",
                        sigma.parameter
                    )
                })?
                + 1;
            format!("logpdf(Normal({predictor}, theta[{sigma_index}]), y[row_index])")
        }
        LikelihoodSpec::BernoulliLogit { .. } => {
            format!("logpdf(Bernoulli(bayes_logistic({predictor})), y[row_index])")
        }
        LikelihoodSpec::PoissonLog { .. } => {
            format!("logpdf(Poisson(exp({predictor})), y[row_index])")
        }
    };

    Ok(format!(
        "function (theta, columns, y)\n    log_probability = zero(eltype(theta))\n    @inbounds for row_index in eachindex(y)\n        log_probability += {contribution}\n    end\n    return log_probability\nend\n"
    ))
}

enum Arity {
    Exact(usize),
    AtLeast(usize),
}

impl Arity {
    fn validate(&self, name: &str, actual: usize) -> Result<(), String> {
        let valid = match self {
            Self::Exact(expected) => actual == *expected,
            Self::AtLeast(minimum) => actual >= *minimum,
        };
        if valid {
            Ok(())
        } else {
            Err(format!(
                "Predictor function `{name}` received an invalid argument count ({actual})."
            ))
        }
    }
}

fn emit_number(value: f64) -> Result<String, String> {
    if !value.is_finite() {
        return Err("Predictor constants must be finite.".to_string());
    }
    Ok(format!("{value:?}"))
}

#[cfg(test)]
mod tests {
    use super::compile_predictor;
    use crate::sci::api::bayes::{BayesModelSpec, BinaryOp, Expression, MathFunction};

    fn compile(expression: Expression) -> super::JuliaPredictorKernel {
        let mut spec: BayesModelSpec = serde_json::from_value(serde_json::json!({
            "dataset": { "sourceType": "table", "sourceId": "data" },
            "response": {
                "expression": { "type": "data_variable", "name": "y" },
                "dataVariables": { "y": "response" }
            },
            "predictor": { "type": "number", "value": 0.0 },
            "dataVariables": { "x": "time" },
            "likelihood": {
                "type": "normal",
                "mean": { "source": "predictor" },
                "sigma": { "parameter": "sigma" }
            },
            "parameters": [
                { "name": "a", "constraint": { "type": "real" }, "prior": { "distribution": "normal", "args": [0.0, 1.0] } },
                { "name": "b", "constraint": { "type": "real" }, "prior": { "distribution": "normal", "args": [0.0, 1.0] } },
                { "name": "sigma", "constraint": { "type": "positive" }, "prior": { "distribution": "exponential", "args": [1.0] } }
            ],
            "sampler": { "algorithm": "nuts", "chains": 1, "samples": 10, "warmup": 5, "saveSamples": false },
            "displayFormula": "test"
        }))
        .unwrap();
        spec.predictor = expression;
        compile_predictor(&spec).unwrap()
    }

    #[test]
    fn binds_nonlinear_predictor_to_numeric_indices() {
        let kernel = compile(Expression::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expression::Binary {
                op: BinaryOp::Mul,
                left: Box::new(Expression::Parameter { name: "a".into() }),
                right: Box::new(Expression::Call {
                    function: MathFunction::Exp,
                    args: vec![Expression::Unary {
                        op: crate::sci::api::bayes::UnaryOp::Neg,
                        arg: Box::new(Expression::Binary {
                            op: BinaryOp::Mul,
                            left: Box::new(Expression::Parameter { name: "b".into() }),
                            right: Box::new(Expression::DataVariable { name: "x".into() }),
                        }),
                    }],
                }),
            }),
            right: Box::new(Expression::Number { value: 2.0 }),
        });

        assert_eq!(kernel.columns, ["time"]);
        assert!(kernel.predictor_source.contains("theta[1]"));
        assert!(kernel.predictor_source.contains("theta[2]"));
        assert!(kernel.predictor_source.contains("columns[row_index, 1]"));
        assert!(kernel.predictor_source.contains("exp("));
        assert!(!kernel.predictor_source.contains("time"));
        assert!(
            kernel
                .likelihood_source
                .contains("for row_index in eachindex(y)")
        );
        assert!(kernel.likelihood_source.contains("theta[3]"));
        assert!(kernel.likelihood_source.contains("logpdf(Normal("));
    }
}
