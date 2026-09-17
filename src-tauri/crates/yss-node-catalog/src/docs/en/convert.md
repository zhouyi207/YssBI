# Convert Type

Supports Numeric, Categorical, Ordinal, Binary, Datetime, Text and Identifier. Scalars remain scalar; DataSeries remain DataSeries. The Target Meaning parameter determines the output meaning.

## Parameters

- **Target Meaning**: Auto (default) or any of the seven meanings. Auto is constrained by connected downstream input pins, never guessed from input values.
- **Numeric Representation**: applies to Numeric targets. Auto preserves existing numeric representations, parses text as Float64 and converts Binary to Int64 0/1. Integer uses Int64; Real uses Float64. The physical output type is fixed when constructing the execution expression, never inferred separately per batch.

## Rules

**Values and Levels** edits original codes and labels. Apply the domain after editing. Categorical may inherit a declared Categorical, Ordinal or Binary source domain; other sources require an explicit domain. Ordinal requires explicitly ordered levels, low to high, or an existing Ordinal domain. Values are never assigned ranks by appearance or alphabetical order. Binary mappings require two distinct values and a positive value.

**Calendar Kind** preserves temporal inputs in Auto, or selects Date, Time or Datetime explicitly. Text defaults to Datetime. **Calendar Precision** defaults to microseconds for new values, with seconds, milliseconds and nanoseconds available. **Calendar Format** accepts strftime patterns such as `%d/%m/%Y` or `%d/%m/%Y %H:%M:%S %z`; empty uses ISO text. Source offsets are removed while retaining calendar and clock fields. Precision loss and discarding a nonzero time component fail. Numeric epochs are not inferred.

Auto intersects the types accepted by all downstream input pins while preserving scalar/series shape. A single candidate resolves the target. No downstream connection, or multiple remaining candidates, leaves the target unresolved; connect a typed consumer or choose a target manually. Conflicting requirements block execution. An unconstrained consumer does not select the input meaning.

For example, Text input followed by a Numeric consumer resolves the conversion to Numeric. Constraints propagate through reroutes and generic ports. Rewiring or changing downstream types resolves the target again and invalidates affected cached semantics. The stored parameter remains Auto rather than being replaced by the inferred target.

Nulls remain null. Numeric text permits surrounding whitespace and leading zeros; real parsing also accepts fractions and scientific notation. Integer mode accepts integer text. Overflow, non-finite values, fractional truncation and numeric precision loss fail. `001` may become 1, while `9007199254740993` requires Integer mode rather than silent floating-point rounding.

Without an explicit mapping, Binary conversion accepts numeric 0/1, case-sensitive text `true`/`false`/`0`/`1`, and existing booleans. An explicit domain may map two original codes using a declared positive value. Other physically encoded Binary columns require their declared two-value domain and positive-value mapping. Text conversion formats stored values rather than substituting category labels.

Categorical and Ordinal preserve original codes and validate all non-null values against the declared domain. Unknown or duplicate codes fail. Runtime metadata retains domains, level order and temporal representation through chained conversions, including scalars and materialized lists. Identifier preserves original representation, including leading zeros, wide integers and empty strings; it does not infer numeric meaning or enforce uniqueness. Conversion to Numeric parses original values, never implicit category indices or ordinal ranks.

## Execution

Series conversions preserve the source snapshot and row alignment. DataFusion evaluates lazy expressions in Arrow batches when a result is read, previewed or consumed downstream. Building an expression neither reads the entire column nor edits the dataset. Successful plan construction does not validate every row; later consumption can fail. Filters, projections and LIMIT may reduce the data evaluated.

This node produces derived results without mutating its source dataset. Dataset meaning edits and physical casts remain separate dataset operations.
