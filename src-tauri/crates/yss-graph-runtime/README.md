# Graph runtime

> Status: Current
> Scope: 图解析 facade 与可丢弃的语义缓存
> Canonical owners: 本 crate 的 resolve 与 semantic_cache 模块
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## Semantic cache

Graph Runtime 为最近使用的图保留一个不含本地化文本的完整 `GraphAnalysis` 缓存，容量由 [semantic_cache.rs](src/semantic_cache.rs) 定义。复用前校验解析文档指纹、registry/kernel 指纹和之前实际读取的资源，包括传递函数正文和 absent lookup。解析缓存的身份与执行身份分开：常量名称、函数参数名称、诊断所用 connection ID 和 orphan metadata 会影响解析快照，不能仅凭 `semanticInputHash` 复用。无关资源变化不使该缓存失效。

位置和节点 user label 从当前文档投影，不进入解析缓存身份；资源显示名与语言按当前请求本地化。缓存命中跳过 Schema、类型和函数语义重算，仍执行输入指纹计算、本地化、投影生成以及 Application 的资源/session 重验。缓存只是可丢弃的派生结果，没有文档或历史写权限；解析、哈希、本地化和大对象释放均在缓存锁外完成。
