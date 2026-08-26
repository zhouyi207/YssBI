# Event Begin

Entry point for an **Event** graph. When the graph runs, execution starts here and flows out through **Out**—there is no exec input pin.

## Usage

Place one **Event Begin** at the root of each event-driven subgraph. Connect **Out** to **Sequence**, **Branch**, **Print**, or other exec nodes to define what runs when the event fires. Data-only pipelines do not need this node; it is for control-flow / side-effect graphs.
