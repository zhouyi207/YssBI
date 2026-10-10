# To Ordinal

Convert a scalar or series to Ordinal, preserving original codes and declaring level order. Scalars remain scalar; series retain row order and alignment.

## Parameter

**Levels and Labels** configures original codes and labels from low to high. It defaults to an empty configuration, which can inherit an existing Ordinal domain. Other inputs require explicit levels.

## Conversion rules

Levels are not inferred from appearance or alphabetical order, and ranks do not replace original values. Every non-null input must belong to the declared domain. Duplicate codes and undeclared values fail. Nulls remain null.

## Output and execution

The output meaning is fixed to Ordinal. Level order and labels travel with the result for later conversions to inherit. Materialized inputs are converted immediately; lazy series validate actual values when consumed. This node produces derived results without editing the source dataset.
