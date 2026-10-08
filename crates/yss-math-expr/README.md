# Mathematical expressions

> Status: Current
> Scope: Plain / LaTeX 数学表达式与关系式的中立 AST 和解析边界
> Canonical owners: [公开入口](src/lib.rs)、[AST](src/ast.rs) 与 [解析适配器](src/adapter.rs)
> Update when: 语法子集、符号解释、解析预算或错误契约改变时

本 crate 将 `mathlex` 的解析结果转换为项目自己的 `MathExpr` / `MathRelation`，
不求值、不构造统计模型，也不持有项目、Graph 或 UI 状态。依赖版本由根 Cargo manifest
拥有。约束线性化和数值公式求值属于 [SCI](../yss-sci/README.md)。

`parse_expression` 解析单个表达式；`parse_relations` 拆分顶层逗号与链式比较，
返回各条关系的独立左右表达式。`ParseOptions` 明确输入格式及已知符号。
纯文本保留完整标识符；LaTeX 保护已知的多字母符号，并按已知符号检查唯一分词，
歧义返回 `AmbiguousSymbol`。LaTeX 中未知的裸名称与括号可表示隐式乘法；
显式 `\operatorname{...}` 调用和语法库识别的函数都受同一函数白名单约束。

临时下标名称只属于一次 LaTeX 解析，并避开输入中的编号。适配器在已有的受限 AST
转换中恢复原名，函数准入检查实际函数名；临时前缀不授予调用权限，也不进入公开 AST。
没有独立的 AST 恢复遍历或第二套语法解析器。

输入字节、关系数量、转换节点数和深度的上限由解析适配器拥有；`MAX_RELATIONS`
同时提供给领域调用方。整个关系请求共用预算，链式比较的中间表达式在两侧出现时
分别计入节点数，不通过共享或重复解析重置预算。非有限数字和不支持的语法被拒绝。
调用递归解析器前复用语法库的分词器，检查分组、前缀及右结合幂的嵌套。
分组数量沿用 AST 深度上限；分组和前缀累计的语法嵌套上限为其两倍，
让带花括号的幂保留完整 AST 深度预算；
独立运算分支重置当前语法深度，进入分组时保留外层尚未完成的递归深度。
这个检查只负责资源准入；语法有效性仍由语法库判定。
`MathErrorKind` 与可选字节偏移描述解析失败；领域调用方负责映射为自己的失败类型。

Focused validation:

```sh
cargo test -p yss-math-expr --lib
cargo test -p yss-sci --lib hypothesis::linear_hypothesis::tests::
cargo clippy -p yss-math-expr --lib --no-deps -- -D warnings
cargo fmt -p yss-math-expr --check
```
