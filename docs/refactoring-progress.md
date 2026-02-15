# 前端代码重构进度

## 重构目标

优化前端代码组织结构，提高可维护性、可测试性和代码质量。

## 第一阶段：完善 Services 层和拆分 Hooks ✅

### 1. 完善 Services 层 ✅

#### 新增文件
- `src/services/graph/pin/pinService.ts` - Pin 服务
- `src/services/index.ts` - 统一导出所有服务

#### 修改文件
- `src/services/graph/connection/connectionService.ts` - 添加导入和类声明
- `src/views/EditorView/Pins/PinInput.tsx` - 使用 PinService 替代直接 invoke

#### 成果
- ✅ 所有后端调用统一通过 Service 层
- ✅ 便于添加日志、错误处理、重试逻辑
- ✅ 易于 mock 和单元测试
- ✅ 类型安全

### 2. 拆分 useEditor Hook ✅

#### 新增文件
- `src/features/editor/hooks/useEditorState.ts` - 状态 Hook
- `src/features/editor/hooks/useEditorActions.ts` - 操作 Hook
- `src/features/editor/hooks/useEditor.old.ts` - 旧版本备份

#### 修改文件
- `src/features/editor/hooks/useEditor.ts` - 重构为组合 Hook
- `src/features/editor/hooks/index.ts` - 导出新 Hooks

#### 成果
- ✅ 单一职责：每个 hook 只负责一类功能
- ✅ 性能优化：组件可以只订阅需要的状态
- ✅ 可测试性：每个 hook 可以独立测试
- ✅ 向后兼容：useEditor 仍然提供完整功能

## 第二阶段：明确 Views 和 Features 边界 ✅

### 1. 创建 Node Feature ✅

#### 新增目录结构
```
src/features/node/
├── hooks/
│   ├── useNodeExecution.ts    # 执行状态 Hook
│   ├── useNodeStyle.ts        # 样式配置 Hook
│   └── index.ts
├── utils/
│   ├── nodeClassNames.ts      # 样式工具函数
│   └── index.ts
└── index.ts
```

#### 成果
- ✅ 将节点的业务逻辑从 View 层提取到 Feature 层
- ✅ 执行状态逻辑独立可测试
- ✅ 样式逻辑集中管理

### 2. 重构 Node 组件 ✅

#### 新增文件
- `src/views/EditorView/Nodes/NodeContainer.tsx` - 节点容器组件
- `src/views/EditorView/Nodes/DefaultNodeLayout.tsx` - 默认布局组件
- `src/views/EditorView/Nodes/MathNodeLayout.tsx` - 数学节点布局组件
- `src/views/EditorView/Nodes/Node.old.tsx` - 旧版本备份

#### 修改文件
- `src/views/EditorView/Nodes/Node.tsx` - 重构为组合组件

#### 成果
- ✅ Views 层只负责 UI 渲染
- ✅ 业务逻辑在 Features 层
- ✅ 组件拆分清晰，易于维护
- ✅ 提高可测试性

### 3. 创建架构文档 ✅

#### 新增文档
- `docs/architecture-layers.md` - 架构分层说明
- `docs/features-organization.md` - Features 与 Views 映射关系
- `docs/features-organization-comparison.md` - 组织方式对比

#### 成果
- ✅ 明确了分层职责
- ✅ 定义了依赖规则
- ✅ 提供了最佳实践指南
- ✅ 明确推荐按功能平铺的组织方式

## 下一步计划

### 第三阶段：重新组织 Types ✅ 已完成

#### 目标
- [x] 按领域划分类型定义
- [x] 区分 Domain Types、DTO、UI State
- [x] 创建清晰的类型层次
- [x] 迁移所有类型文件
- [x] 更新所有导入路径
- [x] 删除旧的类型文件

#### 完成
```
src/shared/types/
├── domain/          # 领域模型（与后端一致）
│   ├── node.ts
│   ├── pin.ts
│   ├── connection.ts
│   ├── graph.ts
│   ├── variable.ts
│   ├── project.ts
│   ├── database.ts  ✨ 新增
│   └── index.ts
├── dto/             # 数据传输对象
│   ├── converters.ts
│   └── index.ts
├── ui/              # UI 状态类型
│   ├── common.ts
│   ├── editor.ts
│   ├── layout.ts
│   ├── execution.ts ✨ 新增
│   └── index.ts
├── settings/        # 设置类型（保持不变）
└── index.ts         # 新的主入口
```

#### 迁移统计
- ✅ 修改文件：55 个
- ✅ 删除旧文件：14 个
- ✅ 导入路径替换：~113 次
- ✅ 编译检查：通过
- ✅ 类型检查：通过

### 第四阶段：建立清晰的依赖层次 ⏳ 进行中

#### 目标
- [x] 分析当前依赖关系
- [x] 设计三层架构
- [x] 创建迁移脚本
- [ ] 执行迁移
- [ ] 更新导入路径
- [ ] 验证编译

#### 计划
```
features/
├── core/                    # 核心层（无依赖）
│   ├── schema/             # Schema 管理
│   ├── node-registry/      # 节点注册
│   ├── project/            # 项目数据
│   ├── viewport/           # 视口状态
│   ├── gesture/            # 手势状态
│   ├── log/                # 日志管理
│   └── ui/                 # UI 基础设施
├── domain/                  # 领域层（依赖 core）
│   ├── node/               # 节点领域逻辑
│   ├── execution/          # 执行管理
│   ├── interaction/        # 画布交互
│   └── graph/              # 图管理
└── application/             # 应用层（协调 domain）
    ├── editor/             # 编辑器
    │   ├── core/          # 编辑器核心
    │   ├── operations/    # 编辑器操作
    │   └── layout/        # 编辑器布局
    └── sync/               # 同步管理
```

#### 已完成
- ✅ 创建依赖分析文档 `docs/phase4-dependency-analysis.md`
- ✅ 设计三层架构方案
- ✅ 创建迁移脚本：
  - `phase4-migrate-step1-create-structure.ps1` - 创建目录结构
  - `phase4-migrate-step2-move-core.ps1` - 移动 Core 层
  - `phase4-migrate-step3-move-domain.ps1` - 移动 Domain 层
  - `phase4-migrate-step4-move-application.ps1` - 移动 Application 层
  - `phase4-migrate-step5-update-imports.ps1` - 更新导入路径
  - `phase4-migrate-step6-update-config.ps1` - 更新配置
  - `phase4-migrate-all.ps1` - 完整流程脚本

#### 执行迁移

运行完整迁移流程：
```powershell
# 预览（不实际修改文件）
.\scripts\phase4-migrate-all.ps1 -DryRun

# 执行迁移（会自动创建备份分支）
.\scripts\phase4-migrate-all.ps1

# 跳过备份直接迁移
.\scripts\phase4-migrate-all.ps1 -SkipBackup
```

或者分步执行：
```powershell
.\scripts\phase4-migrate-step1-create-structure.ps1
.\scripts\phase4-migrate-step2-move-core.ps1
.\scripts\phase4-migrate-step3-move-domain.ps1
.\scripts\phase4-migrate-step4-move-application.ps1
.\scripts\phase4-migrate-step5-update-imports.ps1
.\scripts\phase4-migrate-step6-update-config.ps1
```

#### 预期收益
- ✅ 消除循环依赖
- ✅ 清晰的依赖方向（Core → Domain → Application）
- ✅ 更好的可测试性
- ✅ 更好的可维护性
- ✅ 更小的 bundle size

### 第五阶段：继续重构其他 Views（中优先级）

#### 目标
- [ ] 重构 DataView 组件
- [ ] 重构 LogView 组件
- [ ] 重构 PlotView 组件
- [ ] 提取共享的展示组件

### 第六阶段：统一状态管理（低优先级）

#### 目标
- [ ] 减少 Context 使用
- [ ] 全面使用 Zustand
- [ ] 评估 GroupContext 的必要性

## 重构原则

1. **向后兼容**：保持现有 API 不变，避免破坏性更改
2. **渐进式**：逐步重构，每次只改一小部分
3. **测试驱动**：重构前后保持功能一致
4. **文档先行**：先写文档，明确设计意图
5. **备份旧代码**：保留 `.old.ts` 文件以便回滚

## 测试清单

### Services 层
- [x] PinInput 组件使用 PinService
- [ ] 其他组件迁移到 Service 层
- [ ] 添加错误处理测试

### Hooks
- [ ] useEditorState 返回正确的状态
- [ ] useEditorActions 方法正常工作
- [ ] useEditor 向后兼容
- [ ] 性能测试（减少不必要的重渲染）

### Node 组件
- [ ] NodeContainer 正确显示执行状态
- [ ] DefaultNodeLayout 正确渲染
- [ ] MathNodeLayout 正确渲染
- [ ] 性能测试（memo 是否生效）

## 变更日志

### 2024-02-15 - 第四阶段准备

**依赖分析：**
- ✅ 分析当前 features 依赖关系
- ✅ 识别循环依赖问题
- ✅ 设计三层架构方案

**迁移脚本：**
- ✅ 创建 phase4-migrate-step1-create-structure.ps1
- ✅ 创建 phase4-migrate-step2-move-core.ps1
- ✅ 创建 phase4-migrate-step3-move-domain.ps1
- ✅ 创建 phase4-migrate-step4-move-application.ps1
- ✅ 创建 phase4-migrate-step5-update-imports.ps1
- ✅ 创建 phase4-migrate-step6-update-config.ps1
- ✅ 创建 phase4-migrate-all.ps1（一键迁移）

**文档：**
- ✅ 创建依赖分析文档
- ✅ 创建快速开始指南
- ✅ 更新重构进度文档

### 2024-02-15 - 第一阶段

**Services 层：**
- ✅ 创建 PinService
- ✅ 修复 ConnectionService 导入
- ✅ 更新 services/index.ts
- ✅ 重构 PinInput 使用 PinService

**Hooks 拆分：**
- ✅ 创建 useEditorState
- ✅ 创建 useEditorActions
- ✅ 重构 useEditor
- ✅ 更新 hooks/index.ts

### 2024-02-15 - 第三阶段（已完成）

**Types 重组：**
- ✅ 创建 domain/ 目录（领域模型）
  - node.ts, pin.ts, connection.ts, graph.ts, variable.ts, project.ts, database.ts
- ✅ 创建 dto/ 目录（数据传输对象）
  - converters.ts（所有转换器）
- ✅ 创建 ui/ 目录（UI 状态类型）
  - common.ts, editor.ts, layout.ts, execution.ts
- ✅ 迁移所有类型文件
  - 删除旧的 editor/ 目录
  - 删除 layout.ts, loadStatus.ts, logging.ts
  - 更新主 index.ts
- ✅ 批量更新导入路径
  - 创建自动化迁移脚本
  - 更新 55 个文件
  - 替换 ~113 处导入

**文档：**
- ✅ 创建类型迁移指南
- ✅ 创建迁移完成报告
- ✅ 更新快速参考

**Node Feature：**
- ✅ 创建 features/node/ 目录
- ✅ 创建 useNodeExecution hook
- ✅ 创建 useNodeStyle hook
- ✅ 创建 nodeClassNames utils

**Node 组件重构：**
- ✅ 拆分为 NodeContainer、DefaultNodeLayout、MathNodeLayout
- ✅ 重构 Node.tsx 使用新组件
- ✅ 备份旧版本

**文档：**
- ✅ 创建架构分层文档
- ✅ 创建 Features 组织方式对比文档
- ✅ 明确推荐按功能平铺的组织方式

## 关键决策

### 1. Features 组织方式：按功能平铺 ⭐

**决策：** 采用方式 A（按功能平铺），而不是方式 B（按视图分层）

**理由：**
- 代码复用性高
- 功能发现性好
- 依赖关系清晰
- 扩展性强
- 符合业界实践（React、Next.js、FSD）

**详见：** `docs/features-organization-comparison.md`

### 2. Views 层职责：纯展示组件

**决策：** Views 层只负责 UI 渲染，不包含业务逻辑

**理由：**
- 提高可测试性
- 提高可维护性
- 清晰的职责划分
- 便于组件复用

**详见：** `docs/architecture-layers.md`

### 3. Hooks 拆分策略：单一职责

**决策：** 将大 Hook 拆分为多个小 Hook，每个 Hook 只负责一类功能

**理由：**
- 单一职责原则
- 性能优化（减少不必要的重渲染）
- 提高可测试性
- 提高可维护性

**示例：**
- `useEditor` → `useEditorState` + `useEditorActions` + `useEditor`（组合）
- `Node` → `NodeContainer` + `DefaultNodeLayout` + `MathNodeLayout`

## 参考资料

- [Feature-Sliced Design](https://feature-sliced.design/)
- [React Hooks 最佳实践](https://react.dev/learn/reusing-logic-with-custom-hooks)
- [Zustand 文档](https://docs.pmnd.rs/zustand/getting-started/introduction)
- [Clean Architecture](https://blog.cleancoder.com/uncle-bob/2012/08/13/the-clean-architecture.html)
