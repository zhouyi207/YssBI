use serde::Serialize;
use yss_node_protocol::ResolvedSchemaFact;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum GraphSchemaIssue {
    DataDependent,
    UnconnectedInput,
    UnresolvedUpstream,
    MissingResource,
    MissingColumn,
    InvalidParameter,
    ConflictingInputs,
    DependencyCycle,
    UnsupportedResolver,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum GraphSchemaState {
    NotApplicable,
    /// Column membership is resolved only when the relation is consumed.
    Deferred,
    Exact(ResolvedSchemaFact),
    Pending(GraphSchemaIssue),
    Unavailable(GraphSchemaIssue),
    Conflict(GraphSchemaIssue),
    InternalFailure(GraphSchemaIssue),
}

impl GraphSchemaState {
    pub fn exact(&self) -> Option<&ResolvedSchemaFact> {
        match self {
            Self::Exact(fact) => Some(fact),
            _ => None,
        }
    }

    pub fn issue(&self) -> Option<GraphSchemaIssue> {
        match self {
            Self::Deferred => Some(GraphSchemaIssue::DataDependent),
            Self::Pending(issue)
            | Self::Unavailable(issue)
            | Self::Conflict(issue)
            | Self::InternalFailure(issue) => Some(*issue),
            _ => None,
        }
    }

    pub(crate) fn from_issue(issue: GraphSchemaIssue) -> Self {
        match issue {
            GraphSchemaIssue::DataDependent => Self::Deferred,
            GraphSchemaIssue::UnconnectedInput | GraphSchemaIssue::UnresolvedUpstream => {
                Self::Pending(issue)
            }
            GraphSchemaIssue::MissingResource => Self::Unavailable(issue),
            GraphSchemaIssue::UnsupportedResolver => Self::InternalFailure(issue),
            _ => Self::Conflict(issue),
        }
    }
}
