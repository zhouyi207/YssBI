# KDE

Kernel density estimate on numeric **Values** (Gaussian kernel, Silverman bandwidth $h = 1.06\,\sigma\, n^{-1/5}$), evaluated on a 256-point grid.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **In** | Exec input | Control-flow entry |
| **Values** | Input | `Float64` or `Int64` `DataSeries` |
| **Out** | Exec output | Control-flow exit |

## Usage

Running the graph opens the **Plot** window with the KDE curve. Requires at least 2 valid values; bandwidth falls back to 1.0 when sample variance is zero.
