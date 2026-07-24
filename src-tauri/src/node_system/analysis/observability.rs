use super::{CompilationBasis, CompileId};
use crate::node_system::document::{GraphResourcePath, GraphRevision, NodeId};
use crate::node_system::protocol::NodeTypeId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

macro_rules! numeric_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(u64);

        impl $name {
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectSessionId(Box<str>);

impl ProjectSessionId {
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    pub fn unknown() -> Self {
        Self::new("unknown")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

numeric_id!(RunId);
numeric_id!(ParentCallId);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileProvenance {
    pub project_session_id: ProjectSessionId,
    pub graph_path: GraphResourcePath,
    pub basis: CompilationBasis<GraphRevision>,
    pub compile_id: CompileId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrelationContext {
    pub project_session_id: ProjectSessionId,
    pub graph_path: GraphResourcePath,
    pub graph_revision: GraphRevision,
    pub registry_fingerprint: crate::node_system::registry::RegistryFingerprint,
    pub resource_versions: super::ResourceVersionSet,
    pub compile_id: CompileId,
    pub run_id: Option<RunId>,
    pub node_id: Option<NodeId>,
    pub node_type_id: Option<NodeTypeId>,
    pub parent_call: Option<ParentCallId>,
}

impl CorrelationContext {
    pub fn compile(provenance: &CompileProvenance) -> Self {
        Self {
            project_session_id: provenance.project_session_id.clone(),
            graph_path: provenance.graph_path.clone(),
            graph_revision: provenance.basis.graph_revision,
            registry_fingerprint: provenance.basis.registry_fingerprint.clone(),
            resource_versions: provenance.basis.resource_versions.clone(),
            compile_id: provenance.compile_id,
            run_id: None,
            node_id: None,
            node_type_id: None,
            parent_call: None,
        }
    }

    pub fn for_run(mut self, run_id: RunId, parent_call: Option<ParentCallId>) -> Self {
        self.run_id = Some(run_id);
        self.parent_call = parent_call;
        self
    }

    pub fn for_node(mut self, node_id: NodeId, node_type_id: NodeTypeId) -> Self {
        self.node_id = Some(node_id);
        self.node_type_id = Some(node_type_id);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpanKind {
    Snapshot,
    Analysis,
    Lowering,
    Run,
    Operation,
    ResourceAcquire,
    Cleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpanStatus {
    Started,
    Succeeded,
    Failed,
    Cancelled,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TraceValue {
    Boolean(bool),
    Integer(i64),
    Text(Box<str>),
    Redacted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceFieldSensitivity {
    Public,
    UserLiteral,
    ResourceSecret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitiveFieldAction {
    Redact,
    Omit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RedactionPolicy {
    sensitive_fields: SensitiveFieldAction,
}

impl RedactionPolicy {
    pub const fn strict() -> Self {
        Self {
            sensitive_fields: SensitiveFieldAction::Redact,
        }
    }

    pub const fn omit_sensitive() -> Self {
        Self {
            sensitive_fields: SensitiveFieldAction::Omit,
        }
    }

    pub fn apply(
        self,
        sensitivity: TraceFieldSensitivity,
        value: TraceValue,
    ) -> Option<TraceValue> {
        match sensitivity {
            TraceFieldSensitivity::Public => Some(value),
            TraceFieldSensitivity::UserLiteral | TraceFieldSensitivity::ResourceSecret => {
                match self.sensitive_fields {
                    SensitiveFieldAction::Redact => Some(TraceValue::Redacted),
                    SensitiveFieldAction::Omit => None,
                }
            }
        }
    }
}

impl Default for RedactionPolicy {
    fn default() -> Self {
        Self::strict()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanEvent {
    pub kind: SpanKind,
    pub status: SpanStatus,
    pub correlation: CorrelationContext,
    pub fields: BTreeMap<Box<str>, TraceValue>,
}

impl SpanEvent {
    pub fn new(kind: SpanKind, status: SpanStatus, correlation: CorrelationContext) -> Self {
        Self {
            kind,
            status,
            correlation,
            fields: BTreeMap::new(),
        }
    }

    pub fn with_field(
        mut self,
        key: impl Into<Box<str>>,
        value: TraceValue,
        sensitivity: TraceFieldSensitivity,
        policy: RedactionPolicy,
    ) -> Self {
        if let Some(value) = policy.apply(sensitivity, value) {
            self.fields.insert(key.into(), value);
        }
        self
    }
}

pub trait TraceSink: Send + Sync {
    fn record(&self, event: SpanEvent);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopTraceSink;

impl TraceSink for NoopTraceSink {
    fn record(&self, _: SpanEvent) {}
}

pub static NOOP_TRACE_SINK: NoopTraceSink = NoopTraceSink;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::analysis::{ResourceKey, ResourceVersion};
    use crate::node_system::registry::RegistryFingerprint;

    fn provenance() -> CompileProvenance {
        CompileProvenance {
            project_session_id: ProjectSessionId::new("project-session-7"),
            graph_path: GraphResourcePath("events/main".into()),
            basis: CompilationBasis {
                graph_revision: GraphRevision::new(11),
                registry_fingerprint: RegistryFingerprint::from_bytes([3; 32]),
                resource_versions: BTreeMap::from([(
                    ResourceKey::new("functions/shared"),
                    ResourceVersion::new("9"),
                )]),
            },
            compile_id: CompileId::new(13),
        }
    }

    #[test]
    fn correlation_preserves_the_exact_compile_basis() {
        let provenance = provenance();
        let correlation = CorrelationContext::compile(&provenance)
            .for_run(RunId::new(17), Some(ParentCallId::new(19)));

        assert_eq!(
            correlation.project_session_id,
            provenance.project_session_id
        );
        assert_eq!(correlation.graph_path, provenance.graph_path);
        assert_eq!(correlation.graph_revision, provenance.basis.graph_revision);
        assert_eq!(
            correlation.registry_fingerprint,
            provenance.basis.registry_fingerprint
        );
        assert_eq!(
            correlation.resource_versions,
            provenance.basis.resource_versions
        );
        assert_eq!(correlation.compile_id, provenance.compile_id);
        assert_eq!(correlation.run_id, Some(RunId::new(17)));
        assert_eq!(correlation.parent_call, Some(ParentCallId::new(19)));
    }

    #[test]
    fn strict_redaction_keeps_literals_and_resource_secrets_out_of_events() {
        let event = SpanEvent::new(
            SpanKind::Run,
            SpanStatus::Started,
            CorrelationContext::compile(&provenance()),
        )
        .with_field(
            "literal",
            TraceValue::Text("customer supplied text".into()),
            TraceFieldSensitivity::UserLiteral,
            RedactionPolicy::strict(),
        )
        .with_field(
            "credential",
            TraceValue::Text("database-password".into()),
            TraceFieldSensitivity::ResourceSecret,
            RedactionPolicy::strict(),
        );

        let serialized = serde_json::to_string(&event).unwrap();
        assert!(!serialized.contains("customer supplied text"));
        assert!(!serialized.contains("database-password"));
        assert_eq!(event.fields["literal"], TraceValue::Redacted);
        assert_eq!(event.fields["credential"], TraceValue::Redacted);
    }
}
