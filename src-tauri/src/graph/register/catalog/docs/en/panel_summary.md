# Panel Summary

Panel data regression (Stata `xtset` + `xtreg` family). Requires **Entity ID** and **Time ID** (`Categorical` or `Int64`, same length as **Y**).

## Model family (report)

- Fixed effects (Within)
- LSDV
- First difference
- Random effects (RE)
- Hausman test FE vs RE

## Inputs

- **Y**, **X** regressors
- **Entity ID**, **Time ID**
- Optional **Panel Configure** (constant, VCE; default VCE = cluster by entity)

## Output

**Result** (**PanelSummaryResult**) + panel summary window.
