# Problems module

> Status: Current
> Scope: 图诊断展示、定位与 canonical projection
> Canonical owners: Rust 语义快照拥有诊断，本模块只呈现其只读投影
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## Graph Problems

顶层 diagnostics 是 canonical 集合，支持 graph/resource/connection/node/port/parameter。Node diagnostics 只作 Canvas/Details 索引。Problems、Canvas、Details 和 Run Gate 读取同一 projection。

Producer 覆盖 node/parameter/resource、binding/orphan、repeatable minimum、unbound、类型冲突、Schema 状态、连接方向/容量/顺序、literal/conflicting binding、value cycle 和函数依赖/ABI。Nominal 参数使用 registry validator，filter predicate/project columns 按当前输入 Schema 验证。无法构建内部 resolver 结果时使用 typed internal failure，不伪装成空 Schema。

诊断 wire 为 code、messageKey、arguments、severity、blocking、location、related。Rust 定义词汇和模板，frontend 生成模板表并统一本地化，未知模板/缺参数安全回退。单条 blocking 与 severity 独立，aggregate 汇总 canonical blocking；内部 failure 通过 outcome 独立阻止运行。未绑定的必需输入可为 Warning，但明确 blocking=true。

图诊断定义与模板由 `yss-graph-diagnostics` 拥有，代码及模板键使用 `graph.*` 与 `diagnostics.graph.*`。`GraphRuntimeState::from_components`
在构造时校验定义并返回类型化初始化错误。`yss-node-catalog` 只保存节点、端口、参数和分类文本，
不装配诊断词条，也不依赖诊断类型；Graph Runtime 查询节点文本来填充语义投影，诊断仍交给前端 Graph 模板渲染。

Problems 不可手动清空。定位支持 Graph、Node、Pin、Connection、Details 参数字段和已知资源；缺失资源显示 identity，related locations 提供关联跳转。普通 Graph 问题不靠 tracing/Logs 表达。
