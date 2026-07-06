# Print

Write a **Message** string to the application log during graph execution. Execution passes through from **In** to **Out** after logging.

## Pins

| Pin | Direction | Description |
|-----|-----------|-------------|
| **In** | Exec input | Triggers the log write |
| **Message** | Input (optional) | Text to append to the log |
| **Out** | Exec output | Fires after the message is logged |

## Usage

Insert **Print** on an exec chain for debugging or progress markers. Connect **Message** to a **String** constant or string pin, or leave unconnected if the node default suffices. Chain **Out** to the next exec node (**Sequence** step, **Branch**, **View**, etc.).
