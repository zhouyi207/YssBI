# yss-julia-runtime

System Julia discovery adapter.

This crate owns executable candidate discovery, supported-version probing, and hidden-window
command construction. It exposes typed runtime status and errors for an installed Julia,
but does not own worker assets, task execution, scientific contracts, Tauri commands, or Bayes
behavior.
