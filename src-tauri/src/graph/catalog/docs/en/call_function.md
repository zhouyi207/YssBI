# Call Function

Invokes a **function subgraph** bound to this node. Execution enters at **In** and continues from **Out** after the subgraph finishes.

## Usage

Select the target function in the node inspector, wire **In** from upstream exec flow, and connect **Out** to the next step. Function inputs/outputs are exposed as dynamic pins on this node.
