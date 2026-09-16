# 2026-09-16 审查处理记录（进行中）

原始清单：[2026-09-16-review.md](2026-09-16-review.md)。本记录只记录逐项核实与验证进度，不替代架构文档。未完成项仍属于本次任务范围。

| 项目 | 当前判断与进度 |
| --- | --- |
| 1 保存跳过已排队编辑 | 必须修复；保存标志只控制编辑与历史请求准入，移除执行中对 saving 的再次拒绝。保存到队首后读取前序操作完成后的版本；现有协调测试通过，桌面连续编辑/撤销后立即保存验收待完成 |
| 2 WLS/GLS 总平方和 | 必须修复；WLS/GLS/Prais 共用变换空间平方和，按变换后的截距中心化。权重、相关 Sigma、无截距及平移不变性验证通过；同步修正 WLS golden |
| 3 Numeric 相等 | 必须修复；Equal/NotEqual 使用精确数值比较，递归处理列表和记录，六个比较内核 revision 已递增 |
| 4 大整数比较 | 必须修复；共用 numeric_cmp，整数不转浮点，混合数值比较先比较整数部分，再比较小数部分。边界与反向顺序验证通过 |
| 5 判秩失败回退为零秩 | 必须修复；已删除共线性、WLS、GLS、Prais、OLS、IV2SLS、IVLIML 的回退。模型明确拒绝零秩、秩不足与残差自由度不足；OLS 保留类型化分解错误。包内测试及运行时消费方编译通过 |
| 6 判秩容差中间溢出 | 必须修复；已先算相对容差因子，非有限输入/奇异值返回分解错误。真实 SVD 对角矩阵 `1e308` 回归通过 |
| 7 Prais 迭代上限伪装收敛 | 必须修复；只有容差满足才成功，迭代耗尽返回错误，拒绝无效上限与容差 |
| 8 Settings 跨窗口覆盖 | 必须修复；改为字段补丁广播，本地待提交字段覆盖远端同字段更新，保存时重新读取持久化值并合并补丁。删除全量快照广播与抑制开关，主题记忆随切换一起提交。现有存储测试通过；双窗口桌面验收待完成 |
| 9 Graph 快照接纳不一致 | 必须修复；编辑、历史、加载、刷新和保存共用 installGraphSession，统一接纳后再安装两份镜像。项目快照最终提交前复用 canAcceptGraphSession，拒绝同会话旧 revision；现有快照测试通过，桌面迟到快照验收待完成 |
| 10 Settings 运行时校验 | 必须修复；本地存储、事件和本地更新共用 parseSettingsPatch，缺失允许默认值，错误类型拒绝整份输入。覆盖字符串、布尔、语言与标题栏枚举，解析测试通过 |
| 11 definition-only 节点能力发现 | 必须修复能力发现；GUI 创建目录、兼容目录及 AI 搜索共用实际内核可用性过滤，结构/透明节点仍保留。完整声明继续用于已有图解析和缺少内核诊断；本次不扩展尚未实现的 Series 算法，保留产品待办。目录和 AI 消费方测试通过 |
| 12 重复 spec 构造函数 | 已确认完全重复；统一使用 spec，删除 builtin_spec，目录测试通过 |
| 13 无消费者方法和旧类型 | 已核对并删除 replaceResolvedProjection、SettingsEvent、SettingsEventSubscription。安装路径收敛后删除重复 completeSave/applyTransform，基准消费者改用 hydrate，TypeScript 检查通过 |
| 14 Harness 旧格式迁移 | 必须清理；删除 migrations.rs 及调用、user_version 维护和旧格式改写测试。空库创建当前 schema，已有 schema 不匹配返回 InvalidRecord 且保留记录；JSON 列增加有效性约束。3 项 SQLite 测试及 Application 消费方编译通过，未操作实际用户数据库 |
| 15 无效判断和矩阵复制 | 已清理共线性重复判断与复制；Prais 保留有限 rho 的平稳区间截断，拒绝非有限估计，删除不可达 scale 检查，合并变换循环与中间复制 |
| 16 文档漂移 | 已修正 Equal 中英文及 NotEqual 相关说明、TODO 的 IPC 归属，并更新 Linalg、SCI、Kernel README |

## 已完成验证

- `pnpm test:rs:package -p yss-sci-linalg --test contracts`：5 项通过，包含极大尺度与非有限输入。
- `pnpm test:rs:package -p yss-sci --lib regression::collinearity`：4 项通过。
- `pnpm test:rs:package -p yss-sci --lib --tests`：56 项通过，包含模型错误传播、OLS/WLS/IV golden、面板、时序及加权平方和。
- `pnpm test:rs:package -p yss-sci --lib regression::linear_model::prais`：最终变换清理后 2 项通过，包含平移、迭代上限、无效配置与 rho 边界。
- `pnpm test:rs:package -p yss-node-kernel -p yss-node-catalog --lib`：3 项 Kernel、5 项 Catalog 测试通过。
- `pnpm check:rs:package -p yss-graph-execution --lib`：通过。
- `pnpm check:rs:package -p yss-sci-runtime --lib`：通过。
- `pnpm format:rs:package -p yss-sci -p yss-sci-linalg`：已执行。
- `git diff --check`：通过。
- `pnpm test:ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/graphEditing/editorCommands.test.ts`：实际运行 2 个文件，23 项通过。首次命令还列出不存在的 projectPublicationCoordinator.test.ts，该项没有执行，随后改用下方真实文件。
- `pnpm test:ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts`：6 项通过。
- `pnpm check:ts`：通过。
- `pnpm lint:ts`：退出成功，8 条警告均位于本次未修改文件。
- 本轮修改的 10 个 TypeScript 文件已局部格式化。
- `pnpm test:rs:package -p yss-harness-sqlite --lib`：3 项通过，覆盖旧结构拒绝且记录不变、当前结构重开及 JSON 约束、事件序列和工具幂等。
- `pnpm check:rs:package -p yss-application --lib`：通过，确认 Harness 初始化消费方仍传播持久化错误。
- `pnpm format:rs:package -p yss-harness-sqlite`：已执行。
- `pnpm test:ts src/shared/types/settings/parseSettings.test.ts src/features/core/settings/settingsStore.test.ts`：5 项通过。新增测试仅覆盖纯数据解析，未新增 UI 单元测试。
- Settings 修改后 `pnpm check:ts` 通过；初次 Object.hasOwn 标准库不兼容已改为 hasOwnProperty.call，无编译配置变更。
- `pnpm test:rs:package -p yss-application --lib graph::catalog::tests`：7 项通过。兼容目录原测试依赖无内核的 inverse_standardize，改用已实现且要求数值序列的 OLS，保留数值列接纳、文本列拒绝断言。
- `pnpm test:rs:package -p yss-application --lib automation::graph::tests`：5 项通过，覆盖 AI 目录搜索、编辑、保存、执行与结果读取。
- `pnpm format:rs:package -p yss-graph-runtime -p yss-application`：已执行。

尚未运行完整 CI 或桌面人工验收；未新增 UI 单元测试。Graph 桌面验收需确认前序两个编辑/历史操作均进入最终保存结果，以及迟到的同会话旧快照同时被编辑状态和画布拒绝。
已尝试 computer-use 原生桌面入口，`sky.list_apps()` 返回 native pipe unavailable（os error 2），无法连接桌面；这不计为人工验收通过。
Settings 桌面验收需确认两个窗口同时修改不同 AI/外观字段均被保留，以及本地防抖期间收到远端补丁不会清除本地输入。旧存储键删除逻辑已移除，不执行旧设置转换或迁移。

统计公式核对：[statsmodels GLS/WLS centered_tss 实现](https://www.statsmodels.org/v0.14.0/_modules/statsmodels/regression/linear_model.html)。小样本期望值用有理数独立推导，Iris 总平方和按原始 y 和权重计算。
