# To Numeric

Convert a scalar or series to Numeric. Scalars remain scalar; series retain row order and alignment.

## Parameter

**Numeric Representation** defaults to Auto: existing numbers retain their representation, text parses as Float64, and Binary maps to Int64 0/1 using its positive-value mapping. Integer selects Int64; Real selects Float64.

## Conversion rules

Nulls remain null. Numeric text accepts surrounding whitespace and leading zeros; real parsing also accepts fractions and scientific notation. Integer mode accepts integer text and rejects fractional truncation. Overflow, non-finite values and precision loss fail; for example, `9007199254740993` requires Integer mode.

Categorical, Ordinal and Identifier inputs parse their original values, not category indices or ordinal ranks. Annotated Binary inputs use their declared domain and positive value. Datetime does not implicitly become a numeric timestamp.

## Output and execution

The output meaning is fixed to Numeric. Materialized inputs are converted immediately; lazy series validate batches when consumed. Their physical representation is fixed when the conversion expression is built, not separately per batch. This node produces derived results without editing the source dataset.
