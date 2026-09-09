# yss-project-watcher-notify

Concrete `notify` filesystem adapter for `yss-project-watcher`.

This crate owns native recursive observation, event-to-`ProjectChange` mapping, bounded debounce,
worker lifetime, and retryable drain completion. It implements the platform-neutral factory/session
protocol without depending on Tauri, Application state, Project state, Commands, or transport DTOs.

The capacity-one queue carries a rescan token, never a specific last file event. A full slot already
means that all relevant paths must be rescanned; changes arriving while the sink is busy leave a token
for the next scan. Debounce publishes `ProjectChange::RescanRequired`. Relevant directory-root events
are included, while unrelated/read-only/out-of-root events remain filtered before admission.
