# Get Constant

Outputs a named constant defined in the current Event or Function. Add and edit constants in the graph's Details panel, then select one in this node's parameters or insert its reference from that panel.

The output type follows the selected constant. Supported values include scalars, arrays, objects, DataFrames, and DataSeries. Table data is stored with the graph.

Constants are immutable during a run. Editing a constant changes the graph draft; compile again to use the new value and save to persist it. Renaming a constant preserves its references. Deleting a referenced constant produces a graph problem until its nodes are reassigned or removed.
