┌──────────────────────────┐
│ ProjectIOStore           │  ← load / refresh index / load graph 编排
└───────────┬──────────────┘
            │ hydrate / reset
┌───────────▼──────────────┐
│ projectSnapshotBridge    │  ← 图快照跨 store 读取（显式 import）
│ projectClientReset       │  ← 切换项目时清缓存（显式 import）
├──────────────────────────┤
│ VariableStore             │  ← 低频
│ DatabaseStore             │
│ GraphMetaStore            │
│ GraphDataStore            │  ← ★编辑器核心数据（可 undo）
└──────────────────────────┘
