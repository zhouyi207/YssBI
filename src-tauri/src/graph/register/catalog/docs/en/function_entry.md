# Function Entry

System-managed **shell node** that marks where a **Function** graph begins and exposes its **inputs**. It is created automatically with every function graph and **cannot be deleted, copied, or added from the palette** — you may only move it and wire its pins.

## Pins

All pins are a **projection of the function signature** (edit in the function's Detail panel → *Inputs* / exec section), not fixed on the node:

| Pin | Direction | Description |
|-----|-----------|-------------|
| *(exec)* | Exec output | One pin per **exec** entry in the signature (default: **In** → exposed as control-flow start, e.g. **Then**) |
| *(data)* | Output | One pin per **data** function input; read caller-supplied arguments here |

When the signature has **no exec** entries, exec output pins are omitted — the function is evaluated purely by data dependency.

## Usage

Wire each data output into the function body. When the signature includes exec, connect the exec output into the first control-flow step. Add, rename, retype, or reorder signature entries from the Detail panel; pins here (and on every **Call Function** node targeting this function) update automatically while preserving existing connections where possible.
