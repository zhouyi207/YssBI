pub mod panel_did;
pub mod regression;

#[cfg(test)]
mod tests {
    use super::panel_did::{ComputeDidFakeGroupRequest, DidFakeGroupEnginePayload, ExogLabelEntry};
    use super::regression::{OLSModel, OLSResult};
    use serde_json::json;

    #[test]
    fn regression_models_preserve_wire_shape() {
        let model = OLSModel {
            betas: vec![1.0, 2.5],
            has_constant: true,
            variable_specs: vec![],
            kept_indices: Some(vec![0, 2]),
        };
        assert_eq!(
            serde_json::to_value(model).unwrap(),
            json!({
                "betas": [1.0, 2.5],
                "has_constant": true,
                "variable_specs": [],
                "kept_indices": [0, 2]
            })
        );

        let result: OLSResult = serde_json::from_value(json!({
            "title": "OLS",
            "endog_name": "y",
            "model_basic_info": {
                "model_type": "OLS",
                "method": "Least Squares",
                "num_observation": 2,
                "r_squared": 1.0,
                "adj_r_squared": 1.0,
                "f_statistic": 0.0,
                "prob_f_statistic": 1.0,
                "df_model": 1,
                "df_residual": 1,
                "df_total": 2,
                "ss_model": 1.0,
                "ss_residual": 0.0,
                "ss_total": 1.0,
                "ms_model": 1.0,
                "ms_residual": 0.0,
                "ms_total": 0.5,
                "covariance_type": "nonrobust",
                "aic": 0.0,
                "bic": 0.0
            },
            "coefficients": [],
            "diagnostic_info": { "cond_no": 1.0 },
            "betas": [1.0],
            "cov_beta": [[0.25]]
        }))
        .unwrap();
        let serialized = serde_json::to_value(result).unwrap();
        assert_eq!(serialized["title"], "OLS");
        assert_eq!(serialized["cov_beta"], json!([[0.25]]));
        assert!(serialized.get("cov_beta_nonrobust").is_none());
    }

    #[test]
    fn panel_did_request_uses_the_existing_flattened_wire_shape() {
        let request = ComputeDidFakeGroupRequest {
            payload: DidFakeGroupEnginePayload {
                endog: vec![1.0],
                exog_row_major: vec![1.0],
                ncols: 1,
                all_labels: vec![ExogLabelEntry {
                    variable: "did".to_string(),
                    category: None,
                }],
                entity_id: vec![0],
                time_id: vec![0],
                post: vec![1.0],
                treat: vec![1.0],
                did_label: "did".to_string(),
                observed_coef: 1.0,
                constant: true,
                cov_type: "cluster".to_string(),
            },
            n_perm: 99,
            rng_seed: 7,
        };
        let serialized = serde_json::to_value(request).unwrap();
        assert_eq!(serialized["did_label"], "did");
        assert_eq!(serialized["n_perm"], 99);
        assert_eq!(serialized["rng_seed"], 7);
        assert!(serialized.get("payload").is_none());
    }

    #[test]
    fn scientific_models_do_not_import_node_identity_layers() {
        let model_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sci/models");
        let forbidden = [
            ["node", "_system"].concat(),
            ["graph", "::register"].concat(),
            ["Node", "Definition"].concat(),
            ["Pin", "Definition"].concat(),
            ["Pin", "Role"].concat(),
            ["NodeInstance", "Params"].concat(),
        ];
        let mut offenders = Vec::new();
        for name in ["regression.rs", "panel_did.rs"] {
            let source = std::fs::read_to_string(model_root.join(name)).unwrap();
            for (line_index, line) in source.lines().enumerate() {
                for pattern in &forbidden {
                    if line.contains(pattern) {
                        offenders.push(format!("{name}:{}:{pattern}", line_index + 1));
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "scientific model identity dependencies:\n{}",
            offenders.join("\n")
        );
    }
}
