# Frontend Application Boundaries

This document owns the target boundary for frontend dependency direction and
the removal criteria for entries in `FRONTEND_ARCHITECTURE_DEBT`.

## Authority and dependency direction

Rust remains authoritative for project state, persistence, graph execution,
databases, results, and scientific orchestration. Frontend Application owns
use-case coordination between Views, Core projections/UI state, Services, and
validated wire results. Views do not call Services or backend transports
directly, and Core does not call Application, Services, Views, or Tauri.

The ten production layers and their allowed edges are defined by the strict
frontend architecture policy. Cross-layer access outside an unconditional
edge requires one literal canonical module, exported symbol, and any required
exact consumer. Directory membership, barrel membership, package prefixes,
and raw store access do not grant a capability.

## Core surfaces

View access to Core is limited to reviewed readonly projection/query surfaces,
frontend-authoritative UI state, exact DnD constants/types/guards/modifiers,
and the two exact Dockview root-binding consumers. The existing
`WorkbenchDockviewPort` declaration is represented in policy only by its
approved read-member manifest; this document does not introduce an alias or a
second production interface.

Application may coordinate Core publication and control capabilities. App
composition may bind reviewed root/UI capabilities but may not publish
backend-authoritative projections or consume raw stores.

## Services, wire data, and external packages

Services own IPC and platform adapters. Raw `invoke` remains confined to
`src/services/ipc/invokeCommand.ts`, and path dialog access remains confined to
`src/services/platform/pathDialog.ts`. Application may consume only literal
validated wire result/type declarations; wire parsers, normalizers,
serialization helpers, and transport implementations remain Service-owned.

External packages and repository stylesheets require exact declaration scope,
source layer, mode, resource kind, subpath, and consumer policy rows. Asset and
stylesheet resolution failures are fatal audit errors and are never debt.

## Debt removal

Each debt entry identifies one current resolved origin and exact occurrence
count. A boundary correction is complete only when all affected callers use
the final owner in one compiling change and the corresponding debt entries are
deleted. No adapter, dual route, fallback, compatibility facade, wildcard
allowance, or directory exemption is part of this boundary.
