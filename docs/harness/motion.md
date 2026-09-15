# Graph 共同编辑入口的实施设计

> Status: Historical
> Scope: 将 React / Harness 共同 Application 入口的讨论落实到当前图编辑契约
> Canonical owners: Graph 与 Execution、Statistical Harness、Project 与 IPC 文档
> Update when: 保留设计取舍；生产契约更新专项 owner，验收更新实施记录

## Authority 与调用路径

```text
React 编辑命令 ───┐
                 ├─→ Application → Project 当前 GraphDocument + 可逆历史
Harness 工具 ────┘                         │
                                          └─→ Resolve / 结果有效性 / 只读投影
```

不增加独立 Draft 文档容器。当前文档、保存内容指纹、undo/redo 和编辑版本由 Rust 拥有。
React 保存只读文档／语义投影和本地拖动、文本、IME 等临时交互状态；一次手势或 Harness 批次产生一次历史记录。

## 编辑与提交

1. 以 Project instance、图路径及后端编辑会话／revision 识别当前图；请求携带 operation ID 和 typed mutation。
2. 同一图在 Application 按提交顺序协调，读取当前文档和资源依据，在候选中完成整个批次。
3. 结构错误拒绝整批。结构合法但不可运行的状态可以提交，完整诊断进入 Graph Problems。
4. Resolve 与 I/O 不占用 Project 全局数据锁；提交前重验会话、图版本、资源与语义依据。
5. Project 原子更新文档、版本、历史、dirty 依据和有界请求回执，再发布活动通知与投影。
6. 重复请求复用原提交；丢失回复后先按请求身份查询回执，再读取最新投影。Harness 的持久幂等键关联同一 Project operation，账本收尾失败后可只读恢复原节点／端口映射。会话结束或回执过期时不推断提交结果，也不盲目重放。

## Save、Undo 与 Execute

| 操作                   | 边界                                                                         |
| ---------------------- | ---------------------------------------------------------------------------- |
| GUI 编辑 / undo / redo | 只更新当前内存文档；撤销重新使用当前资源解析，不恢复旧语义快照或已释放结果   |
| Assistant 编辑         | 批次自动保存整个当前图，保留撤销历史；文件失败则批次不提交，无需打开图面板   |
| Save                   | 显式持久化捕获的当前版本；成功才更新保存指纹并清理历史；失败保留当前编辑     |
| Execute                | 捕获当前不可变文档与语义依据，后端准备匹配计划；不隐式保存，也不改用磁盘旧图 |
| 关闭视图               | 遵循现有保存／丢弃与 residency 生命周期；关闭视图不撤销已完成的 Harness 提交 |
| 项目替换               | 关闭旧会话准入，拒绝迟到请求与回调；新编辑身份、投影及结果会话重新建立       |

## 投影与交付

snapshot 与 delta 共用严格安装器；增量从明确基线构造完整候选后安装，未变化实体保留引用。
初始化和重连以当前后端读取恢复；缺失基线、坏增量或慢消费者丢失通知均回到只读快照，不重放写命令修复界面。
持续有序交付使用 Channel，图活动通知与 GUI 回执在同一个前端协调入口收敛。
章节 JSON 配置与这一同步链路独立，不用 UI Spec 替代 GraphDocumentPatch。

## 验收入口

当前源码入口、身份和生命周期细节见[Graph 与 Execution](../architecture/GRAPH_AND_EXECUTION.md)、
[Harness](../architecture/STATISTICAL_HARNESS.md)及[IPC](../../src-tauri/crates/yss-application/src/ipc/README.md)。

[实施记录](../reviews/2026-09-15-motion-json-driver-implementation.md)逐项区分已实现、已有自动检查与待验收项。
必须覆盖 GUI / Harness 竞争、批次失败、重复请求、资源变化后的撤销、保存／执行失败和会话重建。
原性能方案使用同机、同图、同视口的 1,000 / 5,000 节点代表性业务图，在 release 桌面记录端到端和分阶段 p50/p95/p99。
用户已明确本轮不需要 release 测试；逐层微基准与 DOM 模拟测量仍只作为局部证据，不据此承诺帧率。
