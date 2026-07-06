# VCE: Cluster (by Entity)

Panel default VCE: cluster-robust standard errors using **Entity ID** from the panel node (not a separate series pin).

## Output

| Pin | Description |
|-----|-------------|
| **VCE** | Entity-cluster VCE constant handle |

## Usage

Connect **VCE** → **Panel Configure** → **Config** on **Panel Summary** / **Panel DID**.

Equivalent to one-way cluster on panel units; the time dimension is not clustered separately unless you choose a non-cluster VCE.
