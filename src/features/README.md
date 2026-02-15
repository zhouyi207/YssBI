# Features 目录结构

本目录采用三层架构组织代码，确保清晰的依赖关系和职责划分。

## 目录结构

\\\
features/
├── core/                    # 核心层（无依赖或最小依赖）
│   ├── schema/             # Schema 管理
│   ├── node-registry/      # 节点注册表
│   ├── project/            # 项目数据管理
│   ├── viewport/           # 视口状态
│   ├── gesture/            # 手势状态
│   ├── log/                # 日志管理
│   └── ui/                 # UI 基础设施
│
├── domain/                  # 领域层（依赖 core）
│   ├── node/               # 节点领域逻辑
│   ├── execution/          # 执行管理
│   ├── interaction/        # 画布交互
│   └── graph/              # 图管理
│
└── application/             # 应用层（协调 domain）
    ├── editor/             # 编辑器
    │   ├── core/          # 编辑器核心
    │   ├── operations/    # 编辑器操作
    │   └── layout/        # 编辑器布局
    └── sync/               # 同步管理
\\\

## 层次说明

### Core 层（核心层）

**职责：** 提供基础设施和核心数据模型

**特点：**
- 无依赖或最小依赖
- 可以被其他层依赖
- 包含最基础的功能

**包含：**
- \schema\ - 后端 schema 管理
- \
ode-registry\ - 节点定义注册
- \project\ - 项目数据（graphs, variables, databases）
- \iewport\ - 视口状态管理
- \gesture\ - 手势状态管理
- \log\ - 日志收集和过滤
- \ui\ - Toast、Modal 等全局 UI

### Domain 层（领域层）

**职责：** 实现业务领域逻辑

**特点：**
- 依赖 Core 层
- 不依赖 Application 层
- 包含领域特定的业务逻辑

**包含：**
- \
ode\ - 节点样式、执行状态等
- \execution\ - 执行状态管理和可视化
- \interaction\ - 画布交互逻辑
- \graph\ - 图的领域逻辑

### Application 层（应用层）

**职责：** 协调各个领域，提供完整的应用功能

**特点：**
- 依赖 Core 和 Domain 层
- 组合多个 domain feature
- 提供用户可见的功能

**包含：**
- \editor\ - 编辑器（组合多个 domain）
  - \core\ - 编辑器核心状态和初始化
  - \operations\ - 编辑器操作（复制、粘贴、删除等）
  - \layout\ - 编辑器布局管理
- \sync\ - 前后端数据同步

## 依赖规则

### ✅ 允许的依赖

- Application → Domain
- Application → Core
- Domain → Core

### ❌ 禁止的依赖

- Core → Domain
- Core → Application
- Domain → Application
- 同层之间的循环依赖

