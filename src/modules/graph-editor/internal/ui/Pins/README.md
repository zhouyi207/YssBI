# Graph Pin UI

`GraphPinController` consumes one normalized Rust editor-projection `PinData` object. Component
callbacks and context-menu capabilities remain separate props and must never be copied into Canvas
interaction state.

Unconnected scalar input ports render `PinInput`. Editing commits a `SetPinValue` Graph Draft
command; it does not mutate authoritative Rust project state until the graph is explicitly saved.
Numeric and text inputs commit on blur, Boolean inputs commit immediately, Enter blurs, and Escape
restores the projected value.

Connection capacity, repeatable-port removal, type information, literal/default values, and port
diagnostics come from the Rust editor projection. The UI must not reconstruct those facts from an
old node-definition registry or infer required inputs from missing frontend metadata.
