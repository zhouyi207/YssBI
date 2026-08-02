use crate::node_system::document::GraphDocument;
use crate::node_system::plan::ExecutionPlan;
use serde::Serialize;
use serde_json::{Map, Value};

/// Serializes any serde value after recursively sorting object keys.
pub fn canonical_json(value: &impl Serialize) -> String {
    let value = serde_json::to_value(value).expect("test value must serialize");
    serde_json::to_string_pretty(&sort_json(value)).expect("canonical JSON must serialize")
}

/// Uses the ordered document representation because `PortAddress` map keys
/// are structured values and therefore cannot be represented as JSON object keys.
pub fn canonical_document(document: &GraphDocument) -> String {
    format!("revision={}\n{document:#?}", document.revision.get())
}

pub fn canonical_analysis(analysis: &impl std::fmt::Debug) -> String {
    format!("{analysis:#?}")
}

/// Plans deliberately contain non-serde, plan-local handles. Their derived
/// debug representation preserves all semantic plan and provenance fields while
/// normalizing the process-monotonic `compile_id`.
pub fn plan_debug_snapshot(plan: &ExecutionPlan) -> String {
    let mut canonical = plan.clone();
    canonical.provenance.compile_id = crate::node_system::analysis::CompileId::new(0);
    format!("{canonical:#?}")
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries = object.into_iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key, sort_json(value)))
                    .collect::<Map<_, _>>(),
            )
        }
        Value::Array(values) => Value::Array(values.into_iter().map(sort_json).collect()),
        other => other,
    }
}
