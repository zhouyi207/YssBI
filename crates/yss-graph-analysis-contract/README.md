# Graph analysis contract

> Status: Current
> Scope: 图分析环境依据与诊断身份
> Canonical owners: [basis.rs](src/basis.rs) 与 [diagnostic.rs](src/diagnostic.rs)
> Update when: 分析环境或诊断定位契约改变时

`GraphAnalysisBasis` 只限定本次解析使用的冻结 Registry 与 Kernel 指纹。
实际读取的资源及缺失读取由 [Graph Resource Contract](../yss-application/src/graph/README.md#module-ownership)
定义，保存在唯一的 `GraphSemanticSnapshot.dependencies()` 中。缓存、执行包与 Application
重验直接消费该依赖记录，不在分析环境或编辑投影中另存版本/观测表。

[Graph Runtime](../yss-graph-runtime/README.md) 将受跟踪的资源读取记录交给语义快照；
[Graph Analysis](../yss-graph-analysis/README.md) 将其绑定到唯一的语义快照，
Editor 投影环境及语义身份。项目资源授权及提交 revision 由 Project/Application 管理，
Execution 的计划依据继续由其资源准备入口核验，不用分析指纹替代授权版本。

本模块只定义可序列化契约，不解析图、不读取项目或资源，也不依赖界面和执行运行时。
诊断契约提供代码、参数、严重度及类型化位置；诊断定义和本地化模板由
[Graph Diagnostics](../yss-graph-diagnostics/README.md) 拥有。
