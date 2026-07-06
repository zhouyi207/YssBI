# Get DataFrame

Load a project **DataFrame** from the database by node instance parameter. The node emits a **DataFrame** reference on the output pin for downstream data nodes.

## Pins

| Pin | Direction | Description |
|-----|-----------|-------------|
| **DataFrame** | Output | Reference to the selected table in the project database |

## Parameters

| Parameter | Description |
|-----------|-------------|
| **DataFrame** | Select which project database table this node loads (node inspector) |

## Usage

Place **Get DataFrame** at the start of a data pipeline. Choose the target table in the node inspector, then wire **DataFrame** to **Decompose DataFrame**, **Filter DataFrame**, regression nodes, or other consumers. Schema propagation uses the selected table so dynamic pins resolve correctly downstream.
