use serde::Serialize;
use yss_graph_protocol::ResolvedSchemaFact;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum GraphSchemaIssue {
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
            Self::Pending(issue)
            | Self::Unavailable(issue)
            | Self::Conflict(issue)
            | Self::InternalFailure(issue) => Some(*issue),
            _ => None,
        }
    }

    pub(crate) fn from_issue(issue: GraphSchemaIssue) -> Self {
        match issue {
            GraphSchemaIssue::UnconnectedInput | GraphSchemaIssue::UnresolvedUpstream => {
                Self::Pending(issue)
            }
            GraphSchemaIssue::MissingResource => Self::Unavailable(issue),
            GraphSchemaIssue::UnsupportedResolver => Self::InternalFailure(issue),
            _ => Self::Conflict(issue),
        }
    }
}
