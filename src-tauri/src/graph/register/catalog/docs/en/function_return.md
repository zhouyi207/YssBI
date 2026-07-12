# Function Return

System-managed **shell node** that marks where a **Function** graph ends and collects its **outputs**. It is created automatically with every function graph and **cannot be deleted, copied, or added from the palette** — you may only move it and wire its pins.

## Pins

All pins are a **projection of the function signature** (edit in the function's Detail panel → *Outputs* / exec section), not fixed on the node:

| Pin | Direction | Description |
|-----|-----------|-------------|
| *(exec)* | Exec input | One pin per **exec** output in the signature (default: **Out**, exposed as the merge point, e.g. **In**) |
| *(data)* | Input | One pin per **data** function output; write the value to return here |

When the signature has **no exec** entries, exec input pins are omitted — return values are pulled from these data inputs by dependency.

## Usage

Feed each data input with the value the function should return. When the signature includes exec, connect the body's final control-flow step into the exec input. The caller's **Call Function** node reads data values back into its own output pins after the subgraph runs. Add, rename, retype, or reorder signature entries from the Detail panel; pins update automatically while preserving existing connections where possible.
