**方向上可以这么理解，但我不建议你把它简单定义成“DuckDB + Polars 全部删掉，换成 DataFusion + Parquet + SQLite”。**

更准确的目标架构应该是：

```text
                     YssBI
                       │
                YssBI Logical Graph
                       │
                Graph Compiler
                       │
          ┌────────────┴────────────┐
          │                         │
          ▼                         ▼
     DataFusion                  Yss Stats
 Relational Engine          Statistical Engine
          │                         ▲
          └──── Arrow RecordBatch ──┘
          │
          ▼
       Parquet
    Dataset Storage

      SQLite
 Metadata / Catalog /
 Project State / History
```

也就是：

```text
DataFusion = 查询、关系代数、优化、执行
Parquet    = 大规模表数据持久化
SQLite     = 元数据和项目状态持久化
Arrow      = DataFusion ↔ Yss Stats 的数据边界
Yss Stats  = 统计计算
```

这个划分我认为比现在同时使用 DuckDB + Polars 更清晰。DataFusion 官方本身的定位就是可扩展 Rust 查询引擎，提供 SQL、LogicalPlan、optimizer、vectorized execution，并原生使用 Arrow；同时原生支持 Parquet 和自定义 `TableProvider`。([Apache DataFusion][1])

### 你现在三个组件的职责可以这样迁移

| 现在                                | 新架构                    | 是否替代      |
| --------------------------------- | ---------------------- | --------- |
| DuckDB query                      | DataFusion             | ✅         |
| DuckDB relational operations      | DataFusion             | ✅         |
| DuckDB internal storage           | Parquet + 自己的数据集层      | ✅ 但不是等价替换 |
| Polars LazyFrame                  | DataFusion LogicalPlan | ✅ 大部分     |
| Polars filter/select/groupby/join | DataFusion             | ✅         |
| Polars Arrow 转换                   | Arrow/DataFusion 原生    | ✅         |
| Polars 特殊 DataFrame API           | 视情况保留                  | ⚠️        |
| SQLite                            | metadata/project state | 新增        |

因此**如果 Polars 在你的项目里主要承担 ETL、filter、select、join、aggregate、lazy execution，那么它确实很可能可以移除。**

DataFusion 官方也明确把 Polars 描述为 DataFrame library，而 DataFusion 更强调可扩展的数据系统构建能力；DataFusion 当前支持 projection/filter pushdown、join reorder 等 optimizer 能力。([Apache DataFusion][2])

---

但这里有一个非常重要的地方：

**不要让 SQLite 保存真正的统计数据表。**

比如你的项目有：

```text
customer.csv
1,000,000 rows × 100 columns
```

导入后建议：

```text
project/
├── project.sqlite
│
└── datasets/
    ├── 01923.parquet
    ├── 82913.parquet
    └── ...
```

SQLite 保存：

```text
dataset
────────────────────────
id
name
parquet_path
row_count
schema
created_at
modified_at

graph
────────────────────────
id
name
json

node
...

project_settings
...

undo_history
...
```

Parquet 保存：

```text
真正的大表数据

age
income
gender
education
...
```

而 DataFusion：

```text
SQLite
   │
   │ 找到 dataset metadata
   ▼
Parquet path
   │
   ▼
DataFusion TableProvider
   │
   ▼
LogicalPlan
   │
   ▼
Optimizer
   │
   ▼
ExecutionPlan
```

DataFusion 本身有 Catalog / Schema / TableProvider 抽象，也允许你实现自己的 catalog，因此以后甚至可以让 YssBI 的 SQLite metadata layer 对接 DataFusion Catalog，而不是维护两套表定义。([Apache DataFusion][3])

---

### 我尤其建议你取消这一层重复

你现在如果是：

```text
YssBI Graph
    ↓
自己的 Graph Plan
    ↓
Polars LazyFrame
    ↓
DuckDB
```

或者：

```text
DuckDB
 ↓ Arrow
Polars
 ↓
Yss Stats
```

很容易出现：

```text
YssBI semantics
     +
Polars semantics
     +
DuckDB semantics
```

三个事实源。

你之前担心的：

```text
compile graph
execute graph
动态 pin
schema inference
类型传播
代码漂移
多事实源
```

实际上都可能受到影响。

更理想的是：

```text
                 YssBI Graph
                      │
                      ▼
                YssBI Semantic IR
                      │
           ┌──────────┴──────────┐
           ▼                     ▼
   DataFusion LogicalPlan   StatisticalOp
           │                     │
           ▼                     │
       DataFusion                │
           │                     │
           └───── Arrow ─────────┘
```

其中：

> **YssBI Graph 是用户层事实源；DataFusion LogicalPlan 是 relational execution 的事实源。**

不要再额外造一套和 DataFusion LogicalPlan 几乎相同的 relational IR。

### Polars 也不要为了“可能以后需要”而保留

如果最终架构是：

```text
Parquet
   ↓
DataFusion
   ↓
Arrow
   ↓
Yss Stats
```

那么再插一个：

```text
Polars
```

通常只会变成：

```text
Parquet
 ↓
DataFusion
 ↓
Arrow
 ↓
Polars
 ↓
Arrow
 ↓
Yss Stats
```

意义不大。

除非你发现某个具体操作：

```text
DataFusion 做起来特别困难
而 Polars 已经非常成熟
```

才把 Polars 当作**特定算子的实现依赖**，而不是核心执行层。

---

## 唯一需要特别小心的是 Parquet 的“可编辑性”

这里是我认为你迁移前必须解决的问题。

DuckDB 是数据库，所以：

```sql
UPDATE data
SET income = 10000
WHERE id = 123;
```

非常自然。

但 Parquet 是 immutable-oriented 文件格式，不适合频繁：

```text
修改一个 cell
删除一行
插入一行
修改一个值
```

你不能把 Parquet 当 SQLite 那样不断原地 UPDATE。

例如 YssBI 用户：

```text
双击 cell

income:
8000 → 9000
```

如果直接修改 Parquet，最笨的办法会变成：

```text
读 parquet
   ↓
修改
   ↓
重新写 parquet
```

大数据集显然不合适。

所以建议你的 Dataset Layer 从第一天就不要定义成：

```rust
Dataset = ParquetFile
```

而应该定义成：

```text
Dataset
  │
  ├── Base snapshot
  │      └── Parquet
  │
  ├── Delta / edits
  │      └── SQLite / Arrow / own delta layer
  │
  └── Metadata
         └── SQLite
```

例如：

```text
原始：

data.parquet

用户修改：
row 100, income = 9000
row 522, gender = "M"
delete row 810

SQLite:
dataset_edits
──────────────────────
row_id | column | value
100    | income | 9000
522    | gender | M

dataset_deleted_rows
──────────────────────
810
```

查询时逻辑上：

```text
Parquet Base
    +
Delta
    ↓
Current Dataset View
    ↓
DataFusion
```

保存/压缩的时候：

```text
Parquet Base
    +
Delta
    ↓
compact
    ↓
New Parquet Base
```

这会让以后：

```text
Undo/Redo
版本历史
数据编辑
autosave
crash recovery
```

全部好设计很多。

---

## 因此，我建议你最终目标不是“三个库替换三个库”

而是明确成 **5 层**：

```text
┌──────────────────────────────────────┐
│               YssBI UI               │
└──────────────────┬───────────────────┘
                   │
                   ▼
┌──────────────────────────────────────┐
│        Graph / Semantic Layer        │
│                                      │
│ Node semantics / schema / typing     │
└──────────────────┬───────────────────┘
                   │ compile
          ┌────────┴────────┐
          ▼                 ▼
┌──────────────────┐ ┌─────────────────┐
│    DataFusion    │ │    Yss Stats    │
│ Relational       │ │ Statistical     │
│ Engine           │ │ Engine          │
└────────┬─────────┘ └────────▲────────┘
         │                    │
         └────── Arrow ───────┘
         │
         ▼
┌──────────────────────────────────────┐
│          Dataset Layer               │
│                                      │
│ Parquet = base data                  │
│ Delta   = edits                      │
│ SQLite  = metadata / graph / state   │
└──────────────────────────────────────┘
```

其中最值得坚持的三个边界是：

```text
DataFusion = relational algebra
Yss Stats  = statistical algebra
Arrow      = execution boundary
```

而：

```text
Parquet + SQLite
```

属于 **storage architecture**，不是计算 architecture。

### 所以我的结论

**是的，如果你确定 YssBI 要走“自己构建分析系统/执行引擎”的路线，我认为可以逐步把核心架构从 `DuckDB + Polars` 收敛到 `DataFusion + Arrow + Parquet + SQLite + Yss Stats`。**

但建议是**逐步替换而不是一次删除 DuckDB/Polars**：

```text
第一阶段
Yss Graph → DataFusion LogicalPlan
                    ↓
                 Arrow
                    ↓
                Yss Stats

第二阶段
用 Parquet Dataset Provider 替换 DuckDB storage

第三阶段
SQLite 接管 metadata / project state

第四阶段
确认 Polars 已无独占功能 → 删除 Polars

第五阶段
确认 DuckDB 已无 storage/query 职责 → 删除 DuckDB
```

尤其 DataFusion 目前已经到 55.0.0，官方仍明确强调它是用于构建数据库和分析系统的 Rust library，并且其内部变化较快，所以 **YssBI 最好只依赖它稳定的公开扩展接口，例如 `TableProvider`、`LogicalPlan`、`ExecutionPlan`，不要侵入 DataFusion internal crates**。([Apache DataFusion][4])

这会比“今天直接把 Cargo.toml 里的 Polars 和 DuckDB 删掉”安全得多。

[1]: https://datafusion.apache.org/?utm_source=chatgpt.com "Apache DataFusion — Apache DataFusion documentation"
[2]: https://datafusion.apache.org/user-guide/introduction.html?utm_source=chatgpt.com "Introduction — Apache DataFusion documentation"
[3]: https://datafusion.apache.org/library-user-guide/catalogs.html?utm_source=chatgpt.com "Catalogs, Schemas, and Tables — Apache DataFusion documentation"
[4]: https://datafusion.apache.org/contributor-guide/architecture.html?utm_source=chatgpt.com "Architecture — Apache DataFusion documentation"
