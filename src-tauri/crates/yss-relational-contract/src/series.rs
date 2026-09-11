use std::{any::Any, fmt, sync::Arc};

use arrow_schema::Field;

use crate::{RelationHandle, RelationLiteral};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

/// The resolved numeric element type is supplied by the compiled specialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericType {
    Int64,
    Float64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SeriesOperand {
    Series(SeriesHandle),
    Scalar(RelationLiteral),
}

/// An adapter-owned expression over a relation's unchanged row domain. Native expressions
/// stay in the adapter; this boundary does not serialize or optimize an expression tree.
pub trait SeriesPlan: Send + Sync {
    fn field(&self) -> &Field;
    fn as_any(&self) -> &dyn Any;
    fn equals(&self, other: &dyn SeriesPlan) -> bool;
}

#[derive(Clone)]
pub struct SeriesHandle {
    relation: RelationHandle,
    plan: Arc<dyn SeriesPlan>,
}

impl SeriesHandle {
    pub(crate) fn new(relation: RelationHandle, plan: Arc<dyn SeriesPlan>) -> Self {
        Self { relation, plan }
    }

    pub fn relation(&self) -> &RelationHandle {
        &self.relation
    }

    pub fn column(&self) -> &str {
        self.plan.field().name()
    }

    pub fn plan(&self) -> &dyn SeriesPlan {
        self.plan.as_ref()
    }

    /// Project the expression, rather than looking up its display name in the base schema.
    pub fn as_relation(&self) -> Result<RelationHandle, crate::RelationError> {
        self.relation.project_series(std::slice::from_ref(self))
    }
}

impl PartialEq for SeriesHandle {
    fn eq(&self, other: &Self) -> bool {
        self.relation == other.relation && self.plan.equals(other.plan.as_ref())
    }
}

impl fmt::Debug for SeriesHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SeriesHandle")
            .field("relation", &self.relation)
            .field("column", &self.column())
            .field("data_type", self.plan.field().data_type())
            .finish_non_exhaustive()
    }
}
