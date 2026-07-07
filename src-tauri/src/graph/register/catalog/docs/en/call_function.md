# Call Function

Invokes a **function subgraph** bound to this node. Pins are **projected from the target function's signature** (edit in the function's Detail panel): each signature entry becomes a pin on this node — **data** pins carry typed values, **exec** pins participate in control flow.

## Exec vs. data-only

Whether this node has exec pins depends on the **function signature**, not a separate flag:

| Signature | Exec pins on Call | How it runs |
|-----------|-------------------|-------------|
| Includes **exec** input(s) | Matching exec input + output pins (default names **In** / **Out**) | Runs when triggered on the exec input; fires exec outputs after the subgraph finishes. |
| **Data only** (no exec in signature) | *none* | Evaluated on demand when a downstream node pulls a data output pin. |

## Inputs

| Pin | Description |
|-----|-------------|
| *(exec)* | One pin per **exec** entry in the function signature (default: **In**) |
| *(data)* | One pin per **data** function input, fed to the subgraph's **Function Entry** |

## Outputs

| Pin | Description |
|-----|-------------|
| *(exec)* | One pin per **exec** output in the signature (default: **Out**) |
| *(data)* | One pin per **data** function output, read from **Function Return** |

## Usage

Drag a function from the sidebar (or pick it via the palette) to create a Call node bound to that function. Wire data inputs from upstream values; read results from data outputs.

- When the signature includes exec pins, connect the exec input from upstream control flow and exec outputs to the next step.
- For data-only functions, connect output data pins to consumers — the function runs when its result is pulled.

Recursion is supported up to a fixed nesting depth guard; exceeding it aborts the run with an error.
