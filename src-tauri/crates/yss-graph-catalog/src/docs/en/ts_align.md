# TS Align

Aligns a **DataFrame** on a regular time grid, filling missing timestamps and rejecting duplicate time keys.

Time column must be **Int64** or **Date**. Output is an **Aligned** `DataFrame` with the same schema.

## Usage

Run before strict time ops such as **TS Diff** or **TS Lag**. Missing timestamps become null; duplicate keys error.
