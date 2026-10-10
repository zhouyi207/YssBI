# Assemble DataFrame

Assemble one or more series as columns, in input-port order. Columns may have different semantic types.

## Usage

Columns must have equal lengths and pair by current row position. Database series from independent sources and mixed database/in-memory inputs are supported. Rows are not automatically matched by key; shorter columns are not padded.

Compatible database inputs retain a lazy projection; other inputs are read into one table. Names derive from source columns or output ports, with suffixes for duplicates. Use Rename DataFrame to adjust names afterward.
