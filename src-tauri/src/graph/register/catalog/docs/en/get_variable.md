# Get Variable

Reads the current value of a project **variable** selected in the node inspector.

## Inputs

| Pin | Description |
|-----|-------------|
| *(inspector)* **Variable** | Variable to read (not a graph pin) |

## Outputs

| Pin | Description |
|-----|-------------|
| **Value** | Current variable contents (`Any` type; schema resolved from variable) |

## Usage

Pick a variable in the node properties, then wire **Value** to consumers (**Convert**, regression **Y**/**X**, etc.). Re-evaluates each graph run with the latest stored value.
