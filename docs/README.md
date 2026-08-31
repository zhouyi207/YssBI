# YssBI Documentation

This directory contains maintained project knowledge. Start with the sections
below; historical investigations and completed planning notes are kept only
when they remain useful to the current architecture.

## Architecture

- [System architecture](architecture/ARCHITECTURE.md)

- [Workbench Dockview architecture](architecture/WORKBENCH_DOCKVIEW_ARCHITECTURE.md)
- [Diagnostics, errors, traces, and output](architecture/DIAGNOSTICS_ERRORS_AND_OUTPUT.md)

## Implementation notes

- [Database runtime](../src-tauri/crates/yss-database-runtime/README.md)
- [SCI synchronous runtime](../src-tauri/crates/yss-sci-runtime/README.md)
- [Julia Bayes worker protocol](../src-tauri/julia/README.md)

## Development

- [Local development workflow](development/LOCAL_WORKFLOW.md)

## History

- [Version history](version/README.md)

## Maintenance

文档以当前源码和运行时契约为准；`docs/version/` 仅保存历史版本说明，不作为现行实现依据。更新架构或 DTO 后，应同步检查本目录内的相对链接与代码路径。
