# yss-project-watcher-notify

Concrete `notify` filesystem adapter for `yss-project-watcher`.

This crate owns native recursive observation, event-to-`ProjectChange` mapping, bounded debounce,
worker lifetime, and retryable drain completion. It implements the platform-neutral factory/session
protocol without depending on Tauri, Application state, Project state, Commands, or transport DTOs.
