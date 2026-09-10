> Status: Accepted Decision
> Contract: Target Architecture
> Scope: 数据引擎迁移的设计依据；不代表当前生产完成状态
> Canonical owners: 可执行事实以源码和测试为准；实施顺序以迁移分析为准
> Update when: 已接受的迁移方向或边界改变时

实施结果见[数据引擎迁移验收](../reviews/2026-09-10-data-engine-migration.md)。以下保留原始设计分析，其中 DuckDB-first 等早期方向由后续 DataFusion 迁移目标细化。

是的，**如果你已经决定“DuckDB 是 YssBI 的统一数据层 + 关系计算引擎 + 大数据执行引擎”，那么 Polars 的必要性会显著下降。** 对你现在这个架构方向，我认为你应该认真考虑：**是否可以把 Polars 从核心依赖降级为可选依赖，甚至最终删除。**

原因是 DuckDB 和 Polars 在很多能力上确实高度重叠：

| 能力                             | DuckDB  | Polars      |
| ------------------------------ | ------- | ----------- |
| Select / Projection            | ✅       | ✅           |
| Filter                         | ✅       | ✅           |
| Join                           | ✅       | ✅           |
| GroupBy                        | ✅       | ✅           |
| Aggregate                      | ✅       | ✅           |
| Sort                           | ✅       | ✅           |
| Window                         | ✅       | ✅           |
| CSV / Parquet                  | ✅       | ✅           |
| Lazy query                     | ✅ 查询优化器 | ✅ LazyFrame |
| Predicate pushdown             | ✅       | ✅           |
| Projection pushdown            | ✅       | ✅           |
| Streaming / larger-than-memory | ✅       | ✅           |
| Arrow                          | ✅       | ✅           |
| SQL                            | ✅       | ❌           |
| 持久化数据库                         | ✅       | ❌           |

Polars 的 Lazy API 本质也是建立 Logical Plan，然后优化并执行；它也支持 streaming engine。([Polars][1])

问题在于你的 YssBI 已经不是一个：

```text
CSV
 ↓
Polars
 ↓
分析
```

的软件了。

你现在越来越接近：

```text
Files / Imported Data
        ↓
      DuckDB
        ↓
 YssBI Logical Plan
        ↓
   DuckDB Query
        ↓
 Arrow RecordBatch
        ↓
 Statistical Kernels
```

在这个模型里，Polars 确实很容易变成“中间再套一层”。

---

## 举个最典型的例子

假设用户：

```text
Dataset
  ↓
Filter age > 30
  ↓
Select income, age, education
  ↓
Group / transform
  ↓
OLS
```

如果使用 Polars：

```text
DuckDB
  ↓
Arrow
  ↓
Polars
  ↓
filter
  ↓
select
  ↓
transform
  ↓
OLS
```

但实际上完全可以变成：

```text
DuckDB

SELECT
    income,
    age,
    education
FROM dataset
WHERE age > 30

  ↓
Arrow Stream
  ↓
OLS
```

因此：

```text
DuckDB → Arrow → Polars → Stats
```

这里 Polars 并没有贡献不可替代的能力。

反而增加了一个：

```text
Arrow representation
     ↓
Polars representation
     ↓
statistics representation
```

的转换边界。

---

# 更关键的是“大数据保证”

你现在的目标已经变成：

> YssBI 应该支持超大数据统计。

那么从架构责任来看：

```text
DuckDB
```

其实比 Polars 更适合作为你的基础执行引擎。

DuckDB Rust 本身可以直接把结果作为 Arrow `RecordBatch` 逐批交给 Rust，而且 DuckDB 官方把 Arrow 作为 Rust bulk result 的原生列式接口。([DuckDB][2])

这样：

```text
1 TB DuckDB
   ↓
projection/filter
   ↓
Arrow Batch
   ↓
OLS.update()
```

非常自然。

而如果你加入 Polars：

```text
1 TB DuckDB
   ↓
Arrow
   ↓
Polars
   ↓
???
```

你反而必须持续考虑：

```text
这个 Polars operator 是否 streaming？
会不会 materialize？
会不会 fallback？
Arrow → Polars 有没有复制？
```

这增加了执行模型的不确定性。

---

# 但是我不会立刻说“Polars 完全没用”

Polars 仍然有几个比较明显的价值。

### 1. 非 SQL 风格的 DataFrame expression

例如用户有非常复杂的：

```rust
col("a")
    .str()
    ...
    .when(...)
    .then(...)
    .otherwise(...)
```

Polars expression DSL 很舒服。

对于开发者而言：

```rust
df.lazy()
    .with_columns(...)
    .filter(...)
    .group_by(...)
```

往往比动态生成复杂 SQL 更容易写。

但是 YssBI 是图节点软件，不是让开发者手写 Polars DSL。

你的用户实际上看到的是：

```text
Filter Node
String Replace Node
Cast Node
Group Node
```

这些最终完全可以编译成 SQL。

因此 **Polars DSL 的人体工程学优势，对你的最终产品并没有那么重要。**

这是一个关键区别。

---

### 2. 内存 DataFrame

Polars 非常适合：

```text
已经在内存中的表格
       ↓
快速 DataFrame transformation
```

例如：

```text
10 MB
100 MB
500 MB
```

的数据。

但你现在既然已经决定：

> 所有数据最终统一进入 DuckDB

那 DuckDB 本身就可以对这些数据执行查询。

所以你不一定需要第二套 DataFrame abstraction。

---

### 3. 某些表达式或算法 Polars 更方便

确实可能出现：

```text
某个 transformation：
Polars 有现成 API
DuckDB SQL 写起来麻烦
```

这时候保留 Polars 很方便。

但这应该是：

```text
Optional execution backend
```

而不是：

```text
Core data layer
```

---

# 我反而越来越建议你的 YssBI 收敛成三层

```text
                   YssBI
                     │
              Logical Graph
                     │
                     ▼
                  Planner
                     │
        ┌────────────┴─────────────┐
        │                          │
        ▼                          ▼
      DuckDB                    Yss Stats
Relational Engine          Statistical Engine
        │                          │
        └──────── Arrow ───────────┘
```

也就是：

### DuckDB 负责

```text
Storage
Scan
Filter
Projection
Join
GroupBy
Aggregate
Sort
Window
Distinct
Missing handling
Cast
String/date transform
Reshape（能表达的）
```

### Yss Stats 负责

```text
OLS
WLS
GLM
Robust SE
Correlation
Covariance
ANOVA
Tests
Distribution
Optimization
Matrix algorithms
Bayesian plugins
...
```

两者之间：

```text
Apache Arrow RecordBatch
```

成为统一的数据交换协议。

这实际上非常干净：

```text
DuckDB = relational algebra

Yss Stats = statistical algebra

Arrow = boundary
```

---

# 那么 Polars 应该放在哪里？

我会把它降级成：

```text
                 Optional
                    │
                    ▼
             Polars Backend
```

例如：

```text
                   Planner
                     │
      ┌──────────────┼──────────────┐
      ▼              ▼              ▼
   DuckDB          Polars         Yss Stats
```

只有当某个节点：

```text
DuckDB 不方便实现
+
Polars 已经有非常优秀实现
```

才发送给 Polars。

而不是每条数据都：

```text
DuckDB → Polars
```

---

# 甚至你可以先尝试完全删除 Polars

我认为这是一个很值得认真评估的方案。

你的核心依赖最后可能只需要：

```toml
duckdb
arrow        # 或使用 duckdb::arrow

faer
ndarray      # 如果确实还需要

statrs / special function / optimization ...
```

架构：

```text
                 Files
       CSV / Parquet / Excel
                  │
                  ▼
                DuckDB
                  │
          SQL Logical Engine
                  │
                  ▼
          Arrow RecordBatch
                  │
                  ▼
              Yss Compute
           /             \
      Statistics       Matrix
       OLS/GLM           faer
```

这会比：

```text
Files
 ↓
DuckDB
 ↓
Arrow
 ↓
Polars
 ↓
ndarray
 ↓
faer
```

少很多层。

---

# 你的 CSV / Parquet 也不一定需要 Polars

你现在 Polars 启用了：

```toml
"csv"
"ipc"
"parquet"
```

但 DuckDB 本身就可以：

```sql
SELECT *
FROM read_csv('a.csv')
```

以及：

```sql
SELECT *
FROM read_parquet('a.parquet')
```

甚至可以直接：

```sql
CREATE TABLE dataset AS
SELECT *
FROM read_parquet('xxx.parquet');
```

因此如果你的目标本来就是：

```text
所有用户数据
→ DuckDB
```

那么：

```text
Polars CSV reader
Polars Parquet reader
```

也变成了重复能力。

DuckDB 自身就定位为可以直接查询 CSV、Parquet、Arrow 等分析型数据源的执行引擎。([DuckDB][3])

---

# 这和你之前一直在清理的“多事实源”其实是同一个问题

如果同时保留：

```text
DuckDB expression
Polars expression
Rust custom transformation
```

例如一个：

```text
Cast string → float
```

可能逐渐变成三份实现：

```text
DuckDB:
CAST(a AS DOUBLE)

Polars:
col("a").cast(Float64)

Rust:
parse::<f64>()
```

再比如：

```text
Drop Missing
```

也可能三份实现。

时间长了非常容易出现：

```text
DuckDB semantics
≠
Polars semantics
≠
Rust semantics
```

尤其这些边界：

```text
NaN
NULL
categorical
decimal
datetime timezone
overflow
string casting
```

都会开始产生微妙差异。

如果你一直希望 YssBI：

> 单一事实源、避免重复逻辑、避免代码漂移

那么 **只选择 DuckDB 作为 relational semantics owner**，反而和你整个项目的方向很一致。

---

# 我认为你可以把“数据计算”和“统计计算”明确切开

这是最漂亮的边界：

```text
WHERE
SELECT
JOIN
GROUP BY
ORDER
CAST
STRING
DATE
WINDOW

        ↓

      DuckDB
```

而：

```text
OLS
WLS
GLM
PCA
ANOVA
t-test
chi-square
survival
Bayes
optimization

        ↓

     Yss Stats
```

这就没有争议了。

例如：

```text
Dataset
 ↓
Filter
 ↓
Cast
 ↓
Join
 ↓
OLS
```

编译成：

```text
           relational segment
                  │
                  ▼

SELECT
    CAST(a AS DOUBLE),
    b,
    y
FROM t1
JOIN t2 ...
WHERE ...

                  │
                  ▼
               DuckDB
                  │
                  ▼
           Arrow batches
                  │
                  ▼
               OLS
```

甚至你都不需要：

```text
Polars Logical Plan
```

因为 **YssBI 自己已经有 Graph / Logical Plan**。

这一点尤其重要。

Polars Lazy 的主要价值是：

```text
用户 expression
     ↓
Polars Logical Plan
     ↓
Optimizer
```

但你的 YssBI 本身就是：

```text
Graph
 ↓
Resolved Projection
 ↓
Logical Execution Plan
```

如果再引入：

```text
YssBI Plan
 ↓
Polars Plan
 ↓
DuckDB Plan
```

实际上是叠了三套 planner。

这可能不值得。

---

# 所以我现在会稍微修改前面对你的建议

之前我们说：

```text
DuckDB = Storage / Query
Polars = DataFrame Compute
Stats = Statistical Compute
```

结合你现在明确的 **超大数据 + streaming + pushdown** 目标，我更推荐：

```text
DuckDB = Storage + Relational Compute

Arrow = Streaming Data Protocol

Yss Stats = Statistical Compute
```

Polars：

```text
optional / removable
```

而不是核心第三根支柱。

---

## 最简单的判断标准

你可以检查 YssBI 里每一个 Polars 调用：

> **“这个操作如果改成 DuckDB SQL，是否损失能力、性能或者代码清晰度？”**

如果答案基本都是：

```text
不会
```

那就可以删。

如果你最终发现只有 5% 的地方：

```text
Polars 明显比 DuckDB 更合适
```

也不一定值得为这 5% 保留一个非常重的依赖。

尤其你的：

```toml
polars
polars-arrow
polars-dtype
```

本身会带来相当大的 Rust dependency graph 和编译成本。

---

### 因此如果让我现在给 YssBI 定方向

我会选择：

```text
                    YssBI Graph
                         │
                         ▼
                  Logical Plan
                         │
                         ▼
                     Planner
                         │
             ┌───────────┴──────────┐
             │                      │
             ▼                      ▼
          DuckDB                 Yss Stats
   relational execution       statistical kernels
             │                      ▲
             │                      │
             └──── Arrow Stream ────┘
```

**先尝试完全不让 Polars 进入核心路径。**

然后实际开发一段时间，如果碰到明确的：

> “DuckDB 无法很好完成，但 Polars 能明显解决”

再把 Polars 作为一个 backend/plugin 加回来。

这比现在因为“DataFrame 软件好像应该有 Polars”而保留它，更符合 YssBI 当前架构。

所以你的感觉基本是对的：

> **对于“DuckDB-first + pushdown-first + Arrow-streaming + 自研统计算法”的 YssBI，Polars 可能已经从核心依赖变成了可删除依赖。**

而且如果最终真的删掉，我认为这不是功能退化，反而可能是一次很有价值的**架构收敛**。

[1]: https://docs.pola.rs/user-guide/concepts/streaming/?utm_source=chatgpt.com "Streaming - Polars user guide"
[2]: https://duckdb.org/docs/current/clients/rust/result_handling?utm_source=chatgpt.com "Handle Results – DuckDB"
[3]: https://www.duckdb.org/docs/current/clients/rust/overview?utm_source=chatgpt.com "Rust Client – DuckDB"
