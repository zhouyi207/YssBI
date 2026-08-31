# yss-julia-runtime

System Julia discovery and installation adapter.

This crate owns executable candidate discovery, supported-version probing, Windows Juliaup
installation, and hidden-window command construction. It exposes typed runtime status and errors,
but does not own worker assets, task execution, scientific contracts, Tauri commands, or Bayes
behavior.
