# Unpivot

Keys are retained identifier columns; Columns are the value columns in requested order. Value columns must have identical physical types and compatible semantic metadata. Variable column defaults to variable, and Value column to value. Both names must be distinct and must not collide with keys.

Each source row expands into the selected columns in order. Include missing values defaults to true; otherwise Null values are omitted. The variable-name column is Text, and the value column preserves the selected values' semantics.
