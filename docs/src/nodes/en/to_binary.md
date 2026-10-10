# To Binary

Convert a scalar or series to Binary. Scalars remain scalar; series retain row order and alignment.

## Parameter

**Value Mapping** defaults to an empty configuration. An explicit mapping declares two distinct original codes and a positive value: the positive value becomes true and the other value becomes false.

## Conversion rules

Without an explicit mapping, accept existing booleans, numeric 0/1, and case-sensitive text `true`, `false`, `0`, `1`. Existing Binary inputs use their declared two-value domain and positive-value mapping. Non-boolean physical Binary inputs require a positive value.

Nulls remain null rather than becoming false. Values outside the mapping and invalid domains fail. Outputs can feed logical nodes such as AND, OR and NOT.

## Output and execution

The output meaning is fixed to Binary. Materialized inputs are converted immediately; lazy series validate actual values when consumed. This node produces derived results without editing the source dataset.
