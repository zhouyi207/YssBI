# To Categorical

Convert a scalar or series to Categorical while preserving original codes. Scalars remain scalar; series retain row order and alignment.

## Parameter

**Values and Labels** optionally declares distinct original codes and their meanings; it defaults to an empty configuration. Empty configuration first inherits a Categorical, Ordinal or Binary source domain and its labels. Without an inheritable domain, it reads the complete input and automatically gives each distinct non-null value a label identical to its code. No manual entries are required.

## Conversion rules

Automatic mapping preserves empty strings, leading zeros in text and exact numeric codes. Nulls do not create categories; empty or all-null input produces an empty domain. Generated entries are sorted by code text, without implying level order.

With an explicit or inherited domain, every non-null input must belong to that domain. Duplicate codes and undeclared values fail. Nulls remain null. Labels travel as metadata; stored values are not replaced by labels or generated category indices.

Automatic and explicitly configured mappings support at most 65,536 categories and 1 MiB of UTF-8 code and label text. Exceeding a limit fails instead of truncating categories.

Categorical does not express level order. Use To Ordinal when levels are ordered.

## Output and execution

The output meaning is fixed to Categorical. Later conversions can inherit its domain and labels. Automatic mapping scans the complete input during node execution; lazy series with an existing domain still validate actual values when consumed. This node produces derived results without editing the source dataset.
