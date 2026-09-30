# Deduplicate Rows

Keys is optional; empty checks all visible columns. Keep defaults to first. Last keeps the last source row for each key; none removes every row belonging to a repeated key. Null keys are grouped together.

Result preserves the retained rows' source order and all columns. Internal row identifiers are excluded from duplicate keys.
