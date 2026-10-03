# To Identifier

Convert a scalar or series to Identifier. Scalars remain scalar; series retain row order and alignment. There are no parameters.

## Conversion rules

Preserve the original representation, including leading zeros, wide integers and empty strings. Nulls remain null. Numeric meaning is not inferred from contents, and uniqueness is not automatically checked.

Identifier expresses intended meaning rather than a primary-key constraint. To obtain numbers later, use To Numeric to parse original values.

## Output and execution

The output meaning is fixed to Identifier, and semantic metadata travels with the result. Materialized inputs are converted immediately; lazy series are checked when consumed. This node produces derived results without editing the source dataset.
