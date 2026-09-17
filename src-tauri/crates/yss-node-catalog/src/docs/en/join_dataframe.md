# Join

Match two lazy DataFrames using explicit left and right key selections. Multiple keys are supported; both lists must have the same length and matching order.

- **Inner**: retain matching combinations.
- **Left / Right**: retain every row on the selected side, filling unmatched values with null.
- **Full outer**: retain rows from both sides, filling unmatched values with null.

Null keys do not match. Duplicate keys produce all matching combinations without deduplication. Both key columns are retained. Duplicate right-side column names receive the configurable `_right` suffix.

Keys require compatible semantics and identical physical types. Results have stable left-row/right-row ordering, with right-only rows last. Inputs share a project session; evaluation and source leases remain valid through consumption. Source datasets are unchanged.
