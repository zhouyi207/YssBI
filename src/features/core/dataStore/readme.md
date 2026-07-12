┌──────────────────────────┐
│ ProjectIOStore           │  ← load / save / export 编排
└───────────┬──────────────┘
            │ hydrate / reset
┌───────────▼──────────────┐
│ projectSnapshotBridge    │  ← export 跨 store 读取（显式 import）
│ projectClientReset       │  ← 切换项目时清缓存（显式 import）
├──────────────────────────┤
│ VariableStore             │  ← 低频
│ DatabaseStore             │
│ GraphMetaStore            │
│ GraphDataStore            │  ← ★编辑器核心数据（可 undo）
│ GraphRuntimeStore         │  ← ★UI / 交互 / 相机（不可 undo）
└──────────────────────────┘

`dataStore.audit.test.ts` 校验 lifecycle 模块对 store hook 的显式 import。
