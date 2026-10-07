# Panel Alignment

Connect a DataFrame and select entity_column and time_column. Time must be Int64, UInt64 (at most i64::MAX), integral Float64 losslessly representable as Int64, or Date32. Keys cannot be null, and entity-time pairs must be unique.

The sorted union of observed times defines a shared ordinal grid. Positive interval is a step in that grid, not a calendar duration. Each entity is filled only between its own first and last observed positions. Times absent from every entity are not introduced. Off-grid observations are rejected.

Output groups entities in first-seen order and sorts time within each group. Original column order, types and metadata are retained; inserted non-key values are null. Output expansion is bounded by the execution memory budget.
