# To Categorical

Convert a scalar or series to Categorical while preserving original codes. Scalars remain scalar; series retain row order and alignment.

## Parameter

**Values and Labels** declares distinct original codes and their meanings; it defaults to an empty configuration. Empty configuration inherits a compatible Categorical, Ordinal or Binary source domain. Plain text or numeric inputs without an inheritable domain require an explicit one.

## Conversion rules

Every non-null input must belong to the declared domain. Duplicate codes and undeclared values fail. Nulls remain null. Labels travel as metadata; stored values are not replaced by labels or generated category indices.

Categorical does not express level order. Use To Ordinal when levels are ordered.

## Output and execution

The output meaning is fixed to Categorical. Later conversions can inherit its domain and labels. Materialized inputs are converted immediately; lazy series validate actual values when consumed. This node produces derived results without editing the source dataset.
