# Set Variable

Writes a value to a project **variable** when exec reaches this node.

## Inputs

| Pin | Description |
|-----|-------------|
| **In** (exec) | Execution trigger |
| **Value** | Data to store (`Any`) |
| *(inspector)* **Variable** | Target variable selector |

## Outputs

| Pin | Description |
|-----|-------------|
| **Out** (exec) | Passes execution after write |
| **Value** | Echo of the written value (passthrough) |

## Usage

Select the variable, connect upstream data to **Value**, and place **In**/**Out** on the exec chain. Downstream nodes can reuse the same **Value** output without another **Get Variable**.
