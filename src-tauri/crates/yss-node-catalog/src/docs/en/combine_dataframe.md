# Assemble DataFrame

Assemble one or more series as columns, in input-port order. Columns may have different semantic types.

## Usage

Lazy series sharing a proven row domain form a lazy projection, evaluated when consumed. Column selection and renaming preserve alignment; independently filtered or limited row domains cannot be aligned by matching lengths.

Materialized series must have equal lengths and remain materialized. Shorter columns are not padded, and materialized lists cannot mix with lazy series. Names derive from source columns or output ports, with suffixes for duplicates. Use Rename DataFrame to adjust names afterward.
