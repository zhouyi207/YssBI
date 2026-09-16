# 2026-09-16 后续审查处理记录（进行中）

原始清单：[review1](2026-09-16-review1.md)。以下以当前代码和本轮实际验证为准；未完成项仍属于本次任务范围。

| 项目 | 必要性、处理与状态 |
| --- | --- |
| 1 连续参数编辑覆盖 | 必须修复。已删除入队前读取 Store 并合并参数的逻辑，复用 mutation(document) 在任务执行时合并最新参数。适配现有测试，未新增 UI 单元测试；桌面连续参数编辑验收尚未完成。 |
| 2 关闭与在途编辑冲突 | 已实现关闭前 Graph FIFO 屏障并重读 dirty/版本；卸载进入同一队列，后端普通释放保护 dirty 文档，明确丢弃必须匹配确认的 GraphEditVersion。前端仅在后端确认和 lifecycle 仍有效时清除文档与投影，拒绝/失败则保留；重新打开仍受 lifecycle 保护。后端回归和现有前端测试通过，桌面连续编辑后关闭验收待完成。 |
| 3 未加载调用图引用未更新 | 已修复。rename 在文件系统 lease 内枚举文件索引和驻留图，分别改写当前正文和磁盘正文，统一事务提交并重验 authority/资源版本。未加载调用图保持未加载；当前已删除但保存文件仍有的引用也更新，且不顺带保存未提交正文。 |
| 4 动态端口来源及历史未重映射 | 已修复。graph_references 共用实现覆盖当前/已保存文档及 undo/redo 的节点和端口绑定；FunctionParameter 改写保留端口实例、连接与字面量地址。实际 Application 语义解析确认原端口继续使用、无新增端口与 orphan 诊断。 |
| 5 普通文本误作引用 | 已修复。只重写 call.target、entry/return.function 及类型化动态来源。重命名、复制、历史共用规则；删除旧的任意字符串遍历、duplicate_binding/duplicate_locator。复制仍独立生成实例身份，普通文本和字面量不变。 |
| 6 OLS Wald 分解失败生成 F=0 | 已修复。OLS/WLS 共用整体检验实现；协方差 Cholesky 失败或 Wald 非有限/负值明确报错，OLS 使用 OlsFitError::Inference，不生成正常检验结果。 |
| 7 WLS 稳健整体检验仍用经典公式 | 已修复。nonrobust 使用均方比，其他已选协方差使用排除截距的 Wald/F；与 OLS 共用整体检验。单斜率 HC1 的 F=t² 和 p 值已验证。 |
| 8 未知协方差静默降级 | 已修复。WLSConfig 使用现有 OlsCovariance；compute_cov_beta 的字符串入口复用 OlsOptions 解析并穷尽匹配枚举，删除默认降级分支。现有 IV 测试的无效 robust 名称改为 HC1，原测试此前没有真正覆盖稳健计算。 |
| 9 GLS Sigma 与分布不一致 | 已修复并明确现有 API 的 Sigma 为相对协方差结构，残差估计公共尺度；参数协方差包含尺度、系数用 Student-t、整体用 F，报告标记 estimated scale。统一入口单位 Sigma 与 OLS 推断一致。未增加完整已知协方差模式。对角结构与 WLS 一致、相关结构的比例缩放不变性及统一入口验证通过。 |

## 本轮验证

- `pnpm test:ts src/features/application/editor/setNodeParameters.test.ts src/features/application/graphEditing/graphEditCoordinator.test.ts`：6 项通过；只适配原有参数命令测试。
- `pnpm check:ts`：通过。
- `pnpm test:rs:package -p yss-sci --test overall_inference --test weighted_statistics --test regression_golden`：第 6、7 项修复后 8 项通过。
- `pnpm test:rs:package -p yss-sci --lib --tests`：最终 59 项通过。首次因遗漏 rank_failures 的配置消费者而编译失败，修正后发现 IV golden 使用未知名称，改为明确 HC1 后全数通过。
- `pnpm check:rs:package -p yss-sci-runtime --lib`：通过，覆盖推断结果的运行时消费者。
- 新增三个后端回归分别保护奇异协方差失败、稳健整体检验、未知/不完整配置拒绝；未新增 UI 单元测试。

- `pnpm test:rs:package -p yss-sci --test weighted_statistics`：3 项通过；覆盖 GLS/WLS 标准误、t/p、置信区间一致性，相关 Sigma 缩放不变性及统一 GLS/OLS 入口推断一致性。
- `pnpm test:rs:package -p yss-node-kernel --lib`：3 项通过；OLS fit/summary 内核实现 revision 从 1 增至 2，使推断行为变化进入能力指纹。GLS/WLS 当前没有安装独立图内核。
- `pnpm test:ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/editor/workbenchPanelClose.test.ts`：18 项通过；适配现有测试的确认版本、后端确认后清理及队列等待契约。生命周期测试补齐 Graph activity 的边界 mock，不新增 UI 测试。
- `pnpm test:rs:package -p yss-project --lib`：40 项通过。新增后端卸载/丢弃回归；原测试中普通卸载隐含丢弃改为明确版本确认；函数签名既有夹具的旧 Int64 改为当前 Numeric 类型。
- `pnpm test:rs:package -p yss-application --lib automation::graph::tests`：5 项通过；`pnpm check:rs:package -p yss-application --lib` 和 `pnpm check:ts` 通过。

公式依据：[statsmodels 整体 F 定义](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.RegressionResults.fvalue.html)、[GLS 协方差结构](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.GLS.html)。本轮尚未执行桌面人工验收和完整 CI。

## 重命名与引用验证

- `pnpm test:rs:package -p yss-project --lib`：42 项通过。两项新增回归分别覆盖未加载调用图的事务/复制引用和保存正文/可逆历史的引用。后续清理仅重算变化历史的字节数、避免无关文档更新保存指纹，再运行 `--lib reference_tests`，2 项通过。
- `pnpm test:rs:package -p yss-application --lib renamed_unloaded_function_caller_keeps_bound_ports_in_semantic_projection`：1 项通过，真实解析重命名后的连接及字面量端口。
- `pnpm test:rs:package -p yss-application --lib graph::`：27 项通过，包含 Graph 目录、打开/替换、执行/结果以及 Harness Graph 消费方。
- `pnpm format:rs:package -p yss-project -p yss-application` 和 `git diff --check` 已完成。

## 尚未完成的验收

本轮再次尝试 `sky.list_apps()`，原生桌面通道返回 `native pipe unavailable`（os error 2），没有可操作的桌面会话。以下不计为通过：连续修改两个参数后立即关闭的保存/丢弃/取消；调用图未打开时重命名后打开；带已有连线和字面量的函数重命名及撤销/重做。后端自动验证不替代这些桌面验收，任务尚未标记完成。
