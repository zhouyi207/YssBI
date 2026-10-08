# Canonical hashes

> Status: Current
> Scope: 带领域分隔的规范 JSON 身份，以及原始文件/字节内容的 SHA-256
> Canonical owners: [编码、散列和错误入口](src/lib.rs)
> Update when: 规范编码、领域分隔、原始内容散列或错误契约改变时

`hash_canonical` 序列化输入后，复用 Serde JSON 的借用原文读取来按键排序所有层级的对象，
再编码为紧凑 JSON。值不经过浮点转换，原接口支持的 128 位整数保留全部数字。
对象字段的声明或插入顺序不影响身份，数组顺序保留；数值和字符串沿用 Serde JSON 的编码。
这一规则适用于结构体和数组中的嵌套对象，不依赖 `serde_json/preserve_order` 构建特性。
散列输入是领域名称的字节长度（大端 `u64`）、领域 UTF-8 字节与规范 JSON 字节。
调用方拥有领域名称、待散列内容及其业务意义；本 crate 不持有缓存、项目或执行状态。

`content_sha256` 和 `content_sha256_reader` 只散列原始字节，不使用领域分隔或 JSON 编码。
后者按固定缓冲区读取，传播读取错误；它用于文件、插件和其他原始产物身份。
`CanonicalEncodingError` 保留 Serde JSON 的错误来源，调用方映射为自己的失败契约。

对象指纹直接使用当前规范键顺序，不保留依赖构建特性或字段顺序的旧编码路径。
项目保存身份、Graph 语义/依赖和执行缓存、Kernel/Registry 定义及 Harness 幂等/审批
均应通过这一入口生成各自身份；原始文件内容身份仍使用字节散列入口。

Focused validation:

```sh
cargo test -p yss-canonical-hash --lib
cargo test -p yss-canonical-hash --lib --features serde_json/preserve_order
cargo clippy -p yss-canonical-hash --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-canonical-hash --check
```

公共规范编码改变时，按[根验证规则](../../.rules)评估全部身份消费者。
