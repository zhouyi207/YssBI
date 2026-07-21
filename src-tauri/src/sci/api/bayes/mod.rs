mod backend;
mod convert;
mod draft;
mod exchange;
mod expression;
mod input_validation;
mod model;
mod result;
mod validation;
mod validators;

pub use backend::*;
pub use convert::draft_to_model_spec;
pub use draft::*;
pub use exchange::*;
pub use expression::*;
pub use input_validation::*;
pub use model::*;
pub use result::*;
pub use validation::*;
pub use validators::validate_draft;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn valid_draft() -> BayesModelDraft {
        BayesModelDraft {
            formula_text: "y \\sim \\operatorname{Normal}\\left(a * x + b, \\sigma\\right)"
                .to_string(),
            raw_response: RawExpression::Symbol { name: "y".into() },
            bound_response: Some(Expression::DataVariable { name: "y".into() }),
            symbols: vec![
                SymbolDraft {
                    name: "y".to_string(),
                    role: SymbolRole::Dependent,
                    inferred_role: SymbolRole::Dependent,
                    user_edited: true,
                },
                SymbolDraft {
                    name: "x".to_string(),
                    role: SymbolRole::Independent,
                    inferred_role: SymbolRole::Independent,
                    user_edited: true,
                },
                SymbolDraft {
                    name: "a".to_string(),
                    role: SymbolRole::Parameter,
                    inferred_role: SymbolRole::Parameter,
                    user_edited: true,
                },
                SymbolDraft {
                    name: "b".to_string(),
                    role: SymbolRole::Parameter,
                    inferred_role: SymbolRole::Parameter,
                    user_edited: true,
                },
            ],
            dataset: Some(DatasetSelection {
                source_type: DatasetSourceType::Table,
                source_id: "demo".to_string(),
                columns: vec![
                    ColumnMeta {
                        name: "response".to_string(),
                        dtype: ColumnDType::Number,
                        nullable: false,
                    },
                    ColumnMeta {
                        name: "time".to_string(),
                        dtype: ColumnDType::Number,
                        nullable: false,
                    },
                ],
            }),
            response_binding: Some(ResponseBinding {
                symbol: "y".to_string(),
                column: "response".to_string(),
            }),
            data_bindings: BTreeMap::from([("x".to_string(), "time".to_string())]),
            bound_predictor: Some(Expression::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expression::Binary {
                    op: BinaryOp::Mul,
                    left: Box::new(Expression::Parameter {
                        name: "a".to_string(),
                    }),
                    right: Box::new(Expression::DataVariable {
                        name: "x".to_string(),
                    }),
                }),
                right: Box::new(Expression::Parameter {
                    name: "b".to_string(),
                }),
            }),
            likelihood: LikelihoodSpec::Normal {
                mean: PredictorSource {
                    source: PredictorSourceKind::Predictor,
                },
                sigma: ParameterRef {
                    parameter: "sigma".to_string(),
                },
            },
            parameters: vec![
                ParameterSpec {
                    name: "a".to_string(),
                    constraint: ParameterConstraint::Real,
                    prior: PriorSpec::Normal([0.0, 10.0]),
                },
                ParameterSpec {
                    name: "b".to_string(),
                    constraint: ParameterConstraint::Real,
                    prior: PriorSpec::Normal([0.0, 10.0]),
                },
                ParameterSpec {
                    name: "sigma".to_string(),
                    constraint: ParameterConstraint::Positive,
                    prior: PriorSpec::Exponential([1.0]),
                },
            ],
            sampler: InferenceConfig {
                algorithm: SamplerAlgorithm::Nuts,
                chains: 4,
                samples: 2_000,
                warmup: 1_000,
                seed: Some(1234),
                target_accept: Some(0.8),
                max_tree_depth: Some(10),
                save_samples: true,
            },
        }
    }

    #[test]
    fn converts_valid_draft_to_model_spec() {
        let spec = draft_to_model_spec(valid_draft()).expect("valid draft");
        assert_eq!(
            spec.response.data_variables.get("y"),
            Some(&"response".to_string())
        );
        assert_eq!(spec.data_variables.get("x"), Some(&"time".to_string()));
        assert_eq!(spec.parameter_names().len(), 3);
    }

    #[test]
    fn reports_missing_dataset() {
        let mut draft = valid_draft();
        draft.dataset = None;
        let report = validate_draft(&draft);
        assert!(!report.ok);
        assert!(
            report
                .errors
                .iter()
                .any(|issue| issue.code == "DATASET_REQUIRED")
        );
    }

    #[test]
    fn parses_safe_expression_to_raw_ast() {
        let known = vec!["y".into(), "a".into(), "x".into(), "b".into()];
        let parsed =
            parse_model_expression("y = a * x + b", crate::math::ParseOptions::plain(&known))
                .expect("parsed expression");
        assert_eq!(
            parsed.formula.raw_response,
            RawExpression::Symbol { name: "y".into() }
        );
        assert_eq!(parsed.symbols, vec!["a", "b", "x", "y"]);
        assert!(matches!(
            parsed.formula.raw_predictor,
            RawExpression::Binary {
                op: BinaryOp::Add,
                ..
            }
        ));
    }

    #[test]
    fn rejects_unsupported_expression_function() {
        let known = vec!["y".into(), "x".into()];
        let error = parse_model_expression("y = eval(x)", crate::math::ParseOptions::plain(&known))
            .expect_err("unsupported function rejected");
        assert!(error.to_string().contains("不支持函数"));
    }

    #[test]
    fn validates_likelihood_response_dtype() {
        let mut draft = valid_draft();
        if let Some(dataset) = &mut draft.dataset {
            dataset.columns[0].dtype = ColumnDType::String;
        }
        let report = validate_draft(&draft);
        assert!(!report.ok);
        assert!(
            report
                .errors
                .iter()
                .any(|issue| issue.code == "LIKELIHOOD_RESPONSE_TYPE_INVALID")
        );
    }

    #[test]
    fn validates_expression_function_arity() {
        let mut draft = valid_draft();
        draft.bound_predictor = Some(Expression::Call {
            function: MathFunction::Ln,
            args: vec![
                Expression::DataVariable {
                    name: "x".to_string(),
                },
                Expression::Parameter {
                    name: "a".to_string(),
                },
            ],
        });
        let report = validate_draft(&draft);
        assert!(!report.ok);
        assert!(
            report
                .errors
                .iter()
                .any(|issue| issue.code == "EXPRESSION_FUNCTION_ARITY_INVALID")
        );
    }

    #[test]
    fn rejects_multiple_response_data_symbols() {
        let mut draft = valid_draft();
        draft.bound_response = Some(Expression::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expression::DataVariable { name: "y".into() }),
            right: Box::new(Expression::DataVariable { name: "x".into() }),
        });
        let report = validate_draft(&draft);
        assert!(
            report
                .errors
                .iter()
                .any(|issue| { issue.code == "RESPONSE_DATA_SYMBOL_COUNT_INVALID" })
        );
    }

    #[test]
    fn validates_prior_args_and_bounds() {
        let mut draft = valid_draft();
        draft.parameters[0].constraint = ParameterConstraint::Bounded {
            lower: 10.0,
            upper: 0.0,
            include_lower: false,
            include_upper: false,
        };
        draft.parameters[1].prior = PriorSpec::Normal([0.0, -1.0]);
        let report = validate_draft(&draft);
        assert!(!report.ok);
        assert!(
            report
                .errors
                .iter()
                .any(|issue| issue.code == "PARAMETER_BOUNDS_INVALID")
        );
        assert!(
            report
                .errors
                .iter()
                .any(|issue| issue.code == "PARAMETER_PRIOR_ARGS_INVALID")
        );
    }
}
