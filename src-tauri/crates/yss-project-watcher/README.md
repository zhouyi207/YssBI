# yss-project-watcher

Platform-neutral project file-watcher lifecycle and delivery protocol.

This crate owns watcher epochs, admission filtering, session shutdown/drain, retryable timeout
ownership, and the state machine that replaces one watched project with another. It consumes
canonical `yss-project-change` facts and resolves a project root through `yss-project-filesystem`.

It deliberately does not own a filesystem backend, `notify`, ProjectState reconciliation, Tauri
events, commands, or transport DTOs. Concrete filesystem observation belongs to
`yss-project-watcher-notify`; Application maps delivered changes into Project authority.
