use std::{any::Any, fmt, sync::Arc};

use arrow_schema::Field;

use crate::{RelationError, RelationHandle};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericTolerance {
    absolute: f64,
    relative: f64,
}

impl NumericTolerance {
    pub fn new(absolute: f64, relative: f64) -> Result<Self, RelationError> {
        if !absolute.is_finite() || !relative.is_finite() || absolute < 0.0 || relative < 0.0 {
            return Err(RelationError::InvalidInput);
        }
        Ok(Self { absolute, relative })
    }
    pub fn absolute(self) -> f64 {
        self.absolute
    }
    pub fn relative(self) -> f64 {
        self.relative
    }
    /// Treat values within tolerance as equal before applying the comparison operator.
    pub fn evaluate(
        self,
        operation: crate::ComparisonOperation,
        left: f64,
        right: f64,
    ) -> Result<bool, RelationError> {
        let ordering = if self.compare(left, right)? {
            std::cmp::Ordering::Equal
        } else {
            left.partial_cmp(&right)
                .ok_or(RelationError::InvalidInput)?
        };
        Ok(operation.evaluate(ordering))
    }
    pub fn compare(self, left: f64, right: f64) -> Result<bool, RelationError> {
        if !left.is_finite() || !right.is_finite() {
            return Err(RelationError::InvalidInput);
        }
        if left == right {
            return Ok(true);
        }
        let scale = left.abs().max(right.abs());
        let distance = (left - right).abs();
        let threshold = self.absolute + self.relative * scale;
        if distance.is_finite() && threshold.is_finite() {
            return Ok(distance <= threshold);
        }
        // Normalize only the overflow case; inf <= inf would otherwise accept an arbitrary pair.
        Ok((left / scale - right / scale).abs() <= self.relative + self.absolute / scale)
    }
}

#[cfg(test)]
#[test]
fn numeric_tolerance_is_symmetric_finite_and_safe_at_extreme_scales() {
    let tolerance = NumericTolerance::new(1e-12, 1e-9).unwrap();
    for (a, b, expected) in [
        (0.1 + 0.2, 0.3, true),
        (0.0, 1e-13, true),
        (0.0, 1e-6, false),
        (1e100, 1e100 + 1e90, true),
        (f64::MAX, -f64::MAX, false),
    ] {
        assert_eq!(tolerance.compare(a, b).unwrap(), expected);
        assert_eq!(tolerance.compare(b, a).unwrap(), expected);
    }
    assert!(
        !NumericTolerance::new(0.0, 0.0)
            .unwrap()
            .compare(0.1 + 0.2, 0.3)
            .unwrap()
    );
    assert!(
        !NumericTolerance::new(f64::MAX, 0.5)
            .unwrap()
            .compare(f64::MAX, -f64::MAX)
            .unwrap()
    );
    assert!(
        NumericTolerance::new(0.0, 2.0)
            .unwrap()
            .compare(f64::MAX, -f64::MAX)
            .unwrap()
    );
    assert!(tolerance.compare(f64::NAN, 0.0).is_err());
    assert!(tolerance.compare(f64::INFINITY, f64::INFINITY).is_err());
    assert!(NumericTolerance::new(-1.0, 0.0).is_err());
    assert!(NumericTolerance::new(0.0, f64::INFINITY).is_err());
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Logarithm,
    Ln,
    Log2,
    Log10,
    Square,
    Sqrt,
}

impl NumericOperation {
    pub fn requires_float(self) -> bool {
        !matches!(self, Self::Add | Self::Subtract | Self::Multiply)
    }

    pub fn is_unary(self) -> bool {
        matches!(
            self,
            Self::Ln | Self::Log2 | Self::Log10 | Self::Square | Self::Sqrt
        )
    }

    pub fn accepts_arity(self, count: usize) -> bool {
        if self.is_unary() {
            count == 1
        } else if self == Self::Add {
            count >= 2
        } else {
            count == 2
        }
    }

    pub fn evaluate_unary_float(self, value: f64) -> Result<f64, RelationError> {
        if !value.is_finite() {
            return Err(RelationError::InvalidInput);
        }
        let result = match self {
            Self::Ln if value > 0.0 => value.ln(),
            Self::Log2 if value > 0.0 => value.log2(),
            Self::Log10 if value > 0.0 => value.log10(),
            Self::Square => value * value,
            Self::Sqrt if value >= 0.0 => value.sqrt(),
            _ => return Err(RelationError::InvalidInput),
        };
        if result.is_finite() {
            Ok(result)
        } else {
            Err(RelationError::NonFiniteResult)
        }
    }

    /// Shared real-valued arithmetic rules for materialized and batch execution.
    pub fn evaluate_float(self, left: f64, right: f64) -> Result<f64, crate::RelationError> {
        if !left.is_finite() || !right.is_finite() {
            return Err(RelationError::InvalidInput);
        }
        let result = match self {
            Self::Add => left + right,
            Self::Subtract => left - right,
            Self::Multiply => left * right,
            Self::Divide if right == 0.0 => return Err(RelationError::DivisionByZero),
            Self::Divide => left / right,
            Self::Power => {
                if (left == 0.0 && right <= 0.0) || (left < 0.0 && right.fract() != 0.0) {
                    return Err(RelationError::InvalidInput);
                }
                left.powf(right)
            }
            Self::Logarithm => {
                if left <= 0.0 || right <= 0.0 || right == 1.0 {
                    return Err(RelationError::InvalidInput);
                }
                left.log(right)
            }
            Self::Ln | Self::Log2 | Self::Log10 | Self::Square | Self::Sqrt => {
                return Err(RelationError::InvalidInput);
            }
        };
        if result.is_finite() {
            Ok(result)
        } else {
            Err(RelationError::NonFiniteResult)
        }
    }
}

/// The resolved numeric element type is supplied by the resolved specialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericType {
    Int64,
    Float64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SeriesOperand {
    Series(SeriesHandle),
    Scalar(yss_data_contract::TabularScalar),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ComparisonOperand {
    Series(SeriesHandle),
    Scalar(yss_data_contract::TabularScalar),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanOperation {
    And,
    Or,
    Not,
}

impl BooleanOperation {
    pub fn arity(self) -> usize {
        if self == Self::Not { 1 } else { 2 }
    }

    /// SQL three-valued logic, shared by scalar and materialized-series kernels.
    pub fn evaluate(self, values: &[Option<bool>]) -> Result<Option<bool>, RelationError> {
        Ok(match (self, values) {
            (Self::Not, [value]) => value.map(|v| !v),
            (Self::And, [Some(false), _] | [_, Some(false)]) => Some(false),
            (Self::And, [Some(true), Some(true)]) => Some(true),
            (Self::Or, [Some(true), _] | [_, Some(true)]) => Some(true),
            (Self::Or, [Some(false), Some(false)]) => Some(false),
            (Self::And | Self::Or, [_, _]) => None,
            _ => return Err(RelationError::InvalidInput),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BooleanOperand {
    Series(SeriesHandle),
    Scalar(Option<bool>),
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
