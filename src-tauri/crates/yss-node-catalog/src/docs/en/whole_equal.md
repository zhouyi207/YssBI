# Whole Value Equal

Compare two complete materialized values and return one Binary scalar without broadcasting.

Lists require equal lengths and corresponding elements. Records require matching keys and values. Nested structures compare recursively, preserving integer precision. Text compares original values; null equals null. This node does not read entire lazy series or dataframes.

Use Equal or Not Equal for per-row comparison results.
