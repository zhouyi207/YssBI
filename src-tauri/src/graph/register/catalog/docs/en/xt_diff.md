# XT Diff

First-differences an **XT Align** output **DataFrame** within each **entity** (panel Stata `D.` semantics).

Keeps only valid differenced rows; numeric columns are differenced over time per entity.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **Aligned DataFrame** | Input | Balanced panel from **XT Align** |
| **Entity Col** | Input | Entity ID column name |
| **Time Col** | Input | Time column name |
| **Diff** | Output | Differenced `DataFrame` |

## Usage

Run **XT Align** first. **Entity Col** and **Time Col** must match the alignment step.
