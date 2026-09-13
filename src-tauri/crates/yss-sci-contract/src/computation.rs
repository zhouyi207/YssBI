use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CategoricalRole {
    General,
    Individual,
    Time,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MissingValuePolicy {
    Listwise,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticalObservationMetadata {
    pub original_observation_count: usize,
    pub used_observation_count: usize,
    pub dropped_null_count: usize,
    pub dropped_nan_count: usize,
    pub missing_value_policy: MissingValuePolicy,
}

#[cfg(test)]
mod tests {
    use super::{MissingValuePolicy, StatisticalObservationMetadata};
    use serde_json::json;

    #[test]
    fn observation_metadata_describes_row_selection_and_missing_value_policy() {
        let metadata = StatisticalObservationMetadata {
            original_observation_count: 10,
            used_observation_count: 7,
            dropped_null_count: 2,
            dropped_nan_count: 1,
            missing_value_policy: MissingValuePolicy::Reject,
        };

        assert_eq!(
            serde_json::to_value(metadata)
                .expect("statistical observation metadata must serialize"),
            json!({
                "originalObservationCount": 10,
                "usedObservationCount": 7,
                "droppedNullCount": 2,
                "droppedNanCount": 1,
                "missingValuePolicy": "reject"
            })
        );
    }
}
