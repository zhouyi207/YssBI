# yss-api

YssBI's Tauri transport boundary.

This crate owns command handlers, wire DTO mapping, transport errors, frontend event delivery, and
the single command registration table. It does not construct application state, platform adapters,
or backend runtimes; those remain in the root composition crate.

Its only public entry point is `invoke_handler`. The `commands`, `schema`, `error`, and `event`
modules remain private so callers cannot create a second transport facade or registration path.
