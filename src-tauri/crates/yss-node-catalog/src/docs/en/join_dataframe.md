# Join

Match two lazy DataFrames using explicit left and right key selections. Multiple keys are supported; both lists must have the same length and matching order.

- **Inner**: retain matching combinations.
- **Left / Right**: retain every row on the selected side, filling unmatched values with null.
- **Full outer**: retain rows from both sides, filling unmatched values with null.
- **Semi / Anti**: retain left rows with / without a matching right key, returning only left columns. Multiple matches do not repeat left rows.

Null keys do not match. For inner/left/right/full joins, duplicate keys produce all matching combinations without deduplication. Both key columns are retained, and duplicate right-side column names receive the configurable `_right` suffix.

Keys require compatible semantics and identical physical types. Results have stable left-row/right-row ordering, with right-only rows last. Inputs share a project session; evaluation and source leases remain valid through consumption. Source datasets are unchanged.
