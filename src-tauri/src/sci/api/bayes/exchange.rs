use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BayesDataExchangeManifest {
    pub version: u32,
    pub task_id: String,
    pub input_table_path: String,
    pub model_spec_path: String,
    pub inference_config_path: String,
    pub predictor_kernel_path: String,
    pub likelihood_kernel_path: String,
    pub predictor_columns: Vec<String>,
    pub output_path: String,
    pub metadata_path: String,
    pub input_rows: usize,
    pub input_columns: Vec<BayesExchangeColumn>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BayesExchangeColumn {
    pub name: String,
}

impl BayesDataExchangeManifest {
    pub const VERSION: u32 = 3;

    pub fn new(
        task_id: impl Into<String>,
        input_table_path: impl Into<String>,
        model_spec_path: impl Into<String>,
        inference_config_path: impl Into<String>,
        predictor_kernel_path: impl Into<String>,
        likelihood_kernel_path: impl Into<String>,
        predictor_columns: Vec<String>,
        output_path: impl Into<String>,
        metadata_path: impl Into<String>,
        input_rows: usize,
        input_columns: Vec<BayesExchangeColumn>,
    ) -> Self {
        Self {
            version: Self::VERSION,
            task_id: task_id.into(),
            input_table_path: input_table_path.into(),
            model_spec_path: model_spec_path.into(),
            inference_config_path: inference_config_path.into(),
            predictor_kernel_path: predictor_kernel_path.into(),
            likelihood_kernel_path: likelihood_kernel_path.into(),
            predictor_columns,
            output_path: output_path.into(),
            metadata_path: metadata_path.into(),
            input_rows,
            input_columns,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BayesDataExchangeManifest, BayesExchangeColumn};

    #[test]
    fn exchange_manifest_uses_stable_camel_case_fields() {
        let manifest = BayesDataExchangeManifest::new(
            "task-1",
            "input.arrow",
            "model_spec.json",
            "inference_config.json",
            "predictor_kernel.jl",
            "likelihood_kernel.jl",
            vec!["x".to_string()],
            "output.arrow",
            "metadata.json",
            2,
            vec![BayesExchangeColumn {
                name: "y".to_string(),
            }],
        );
        let value = serde_json::to_value(&manifest).expect("manifest json");
        assert_eq!(value["version"], 3);
        assert_eq!(value["taskId"], "task-1");
        assert_eq!(value["inputTablePath"], "input.arrow");
        assert_eq!(value["modelSpecPath"], "model_spec.json");
        assert_eq!(value["inferenceConfigPath"], "inference_config.json");
        assert_eq!(value["predictorKernelPath"], "predictor_kernel.jl");
        assert_eq!(value["likelihoodKernelPath"], "likelihood_kernel.jl");
        assert_eq!(value["predictorColumns"][0], "x");
        assert_eq!(value["inputColumns"][0]["name"], "y");
    }
}
