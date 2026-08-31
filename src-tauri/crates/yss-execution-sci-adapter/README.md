# yss-execution-sci-adapter

`yss-sci-runtime` implementation of Execution's live scientific backend port.

The adapter owns the exhaustive ACF/PACF request, result, control, and typed-error mapping between
`yss-execution` and `yss-sci-runtime`. It has no dependency on Tauri, Application, Project, or
Database state.
