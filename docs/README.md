# YssBI Documentation

This directory contains maintained project knowledge. Start with the sections
below; historical investigations and completed planning notes are kept only
when they remain useful to the current architecture.

## Architecture

- [System architecture](architecture/ARCHITECTURE.md)
- [Design rules](architecture/DESIGN_RULE.md)
- [Editor grid architecture](architecture/EDITOR_GRID_ARCHITECTURE.md)
- [SCI architecture](architecture/SCI_ARCHITECTURE_ANALYSIS.md)

## Contracts

- [DTO analysis](contracts/DTO_ANALYSIS.md)
- [DTO type mapping](contracts/DTO_TYPE_MAPPING.md)
- [Frontend/backend interaction](contracts/FRONTEND_BACKEND_INTERACTION.md)

## Features

- [Runtime source lifecycle](features/runtime-source-lifecycle.md)
- [Workbench satellite windows](features/WORKBENCH_SATELLITE_WINDOWS.md)
- [Deferred workbench items](features/WORKBENCH_P3_DEFERRED.md)

## Plans

- [Node protocol and execution architecture](plan/node-architecture.md)
- [Bayesian inference architecture](plan/bayesian-inference.md)
- [Bayesian frontend UI](plan/bayesian-frontend-ui.md)
- [Julia scientific backend](plan/julia.md)

## Maintenance

文档以当前源码和运行时契约为准；`docs/version/` 仅保存历史版本说明，不作为现行实现依据。更新架构或 DTO 后，应同步检查本目录内的相对链接与代码路径。
