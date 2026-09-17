# Concatenate Rows

Append two or more lazy DataFrames in input-port order, preserving duplicate rows and each input's row order. Additional DataFrame input ports can be added.

- **By name** (default): align matching names, fill missing columns with null, and append new columns in first-seen order.
- **By position**: require equal column counts and retain the first input's names and column order.

Corresponding columns require compatible semantic definitions and identical physical types; no implicit lossy conversion is performed. Inputs must share a project session and cannot mix snapshots of the same dataset. Evaluation occurs on consumption without modifying source datasets.
