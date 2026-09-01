┌──────────────────────────┐
│ ProjectIOStore │ ← load / refresh index / load graph 编排
└───────────┬──────────────┘
│ hydrate / reset
┌───────────▼──────────────┐
│ Application project │ ← 图快照组装与项目切换协调
│ projectClientReset │ ← 切换项目时清缓存（显式 import）
├──────────────────────────┤
│ VariableStore │ ← 低频
│ DatabaseStore │
│ GraphMetaStore │
│ GraphDataStore │ ← ★编辑器核心数据（可 undo）
└──────────────────────────┘
