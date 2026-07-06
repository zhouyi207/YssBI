# Panel Configure

Builds a **PanelConfigure** struct for **Panel Summary** and **Panel DID (TWFE)**.

## Inputs

| Pin | Default | Description |
|-----|---------|-------------|
| **Constant** | `true` | Intercept in within/LSDV specs |
| **VCE** | cluster by entity | **VCE: NonRobust** / **HC0–HC3** / **VCE: Cluster (by Entity)** |

## Output

| Pin | Description |
|-----|-------------|
| **Config** | **PanelConfigure** handle |

Wire **Config** to the optional **Config** pin on **Panel Summary** / **Panel DID**. Panel DID options (parallel trends, placebo) are stored in config for report extensions.
