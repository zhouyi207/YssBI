# Database module

`src-tauri/src/database/` 是项目数据资产、DuckDB query/edit semantics、overview 与 export 的 ownership locality。Application orchestration 位于 `src-tauri/src/application/database.rs`，Tauri transport 位于 `src-tauri/src/commands/command_dataframe/`。

`yss-sci` 只承载数值与统计/计量算法，不包含 database edit history、DuckDB state 或 export workflow。

## 1. Storage 与 typed import

项目 tabular 数据统一物化到 `database/project.duckdb`。每个用户数据集对应一个 DuckDB table：

- `ProjectData.databases` 保存 authoritative `DatabaseDecl`；
- `ProjectStore.databases` 保存当前 project session 的 `DatabaseInstance`；
- project activation 枚举 DuckDB 用户表并重建 declaration/runtime projection。

IPC import interface 是 source-only typed enum `DatabaseImportSourceDTO`：

- `Csv { path, delimiter, hasHeader, inferSchemaLength }`
- `Parquet { path, columns }`
- `Excel { path, sheet }`
- `Sql { engine: Sqlite | Postgres | Mysql, connectionString, table }`

Frontend 不能通过 import interface 注入 project-internal `DuckDb` 或 runtime-only `InMemory` engine。Command 将 typed source 转换为内部 `DatabaseEngineDTO`，再交给 application module 执行 ingest、authority commit 与 event publication。

## 2. DuckDB/Polars seam

主要数据交换路径：

- **写入**：Polars `DataFrame` → Arrow RecordBatch → DuckDB Appender；ingest 每批 50,000 rows。
- **读取**：DuckDB `query_arrow` → Arrow → Polars `DataFrame`。
- **分页**：DuckDB `LIMIT/OFFSET` + built-in `rowid`，不整表物化。
- **按列运行资源**：`load_columns` / `load_column_series`，只读取需求列。
- **统计概览**：DuckDB SQL aggregate，避免 DataFrame full load。

`DatabaseInstance` 的主要 state：

```text
DuckDb { path, table, row_count, columns, history }
Loaded { dataframe, original, history }
Failed { error }
```

DuckDB 是普通 production state。只有需要完整 Polars DataFrame 且 table 不超过 50,000 rows 时才进入 `Loaded`。这一 interface 将磁盘列存复杂度隐藏在 module 内，为 page query、graph resource 与 DataView 提供统一 leverage。

## 3. DataView 编辑与 undo

DuckDB-backed DataView edit 使用 SQL 增量 mutation：

- row identity 使用 DuckDB `rowid` pseudo-column；
- cell、row、column、rename、cast 直接执行 SQL；
- undo/redo 使用同一 `EditOperation` 反向/正向执行；
- `save_database_changes` 对 DuckDB state 刷新 metadata 并清空 history，不重建整表。

### 3.1 Reversible delete-column 上限

Delete-column 在真正 drop 前捕获 `DuckDbColumnSnapshot`。这是该 operation 的 undo admission limit，不是 `EditHistory` stack length：

| Limit | Value |
|---|---:|
| `MAX_DELETE_COLUMN_SNAPSHOT_ROWS` | 50,000 rows |
| `MAX_DELETE_COLUMN_SNAPSHOT_BYTES` | 16 MiB |

Snapshot 保存：

- 原 DuckDB storage dtype 对应的 exact editable dtype；
- row IDs；
- 其余列形成的 row fingerprints；
- 被删列 values。

如果 dtype 不能精确恢复、snapshot 超过 row/byte limit，或 snapshot 不完整，drop column 在 mutation 前失败。Undo restore 前再次校验 row count、row IDs 与 fingerprints；只有 table identity 仍与 snapshot 一致才在 transaction 中恢复原 dtype 与 values。

Cast operation 也在 `EditOperation::CastColumn` 中保存 `old_dtype`。In-memory reverse path 使用 `old_dtype + old_data` 重建原列；DuckDB reverse path 将 column cast 回保存的 dtype。

## 4. Identifier 与 checked conversion

DuckDB SQL 将 identifiers 与 literals 分开处理：

- table/column identifier 使用 `quote_duckdb_identifier`，双写内嵌 `"`；
- path/value string 使用 `quote_duckdb_string_literal`，双写内嵌 `'`；
- editable dtype 通过固定 allowlist 映射 SQL type，不拼接任意 type text。

JSON → Polars conversion 严格保留 target dtype：

- narrow signed/unsigned integer 使用 `TryFrom`；
- Float32 先检查 representable range；
- 不兼容 value 返回 error，不静默截断或变更 column dtype。

Row count 与 snapshot size 等跨整数类型/容量计算使用 checked conversion 或 checked addition；overflow 会拒绝 operation。

## 5. Categorical / ENUM

DataView 将列 cast 为 Categorical 后保存时，DuckDB 使用 `_yssbi_enum_{table}_{column}` ENUM type。读写存在物理不对称：

| Path | DuckDB/Arrow behavior | Module adaptation |
|---|---|---|
| Read | ENUM 通过 Arrow Dictionary 返回 | `restore_categorical_columns` 根据 DuckDB schema 恢复 Polars Enum/Categorical |
| Write | DuckDB Appender 接受 category literal，不接受 Dictionary 直写 ENUM | Categorical/Enum 先转换为 String/Utf8 再 append |

因此 ENUM ingest 的 String bridge 是当前 production contract，不是多余 round-trip。

## 6. Export

`DatabaseInstance::export_to_path` 按 state 选择 adapter：

- DuckDB table：执行 `COPY (SELECT * FROM <table>) TO <path>`，CSV 使用 header，Parquet 使用 native format；大表不会先完整进入 Polars。
- Loaded DataFrame：使用 Polars CSV/Parquet writer。

Application export workflow 不直接覆盖 destination：

1. 在 destination 同目录以 `create_new` 保留 unique sibling temp file；
2. database snapshot 导出到 temp；
3. 获取最终 project publication authority；
4. 原子替换 destination；Windows 使用 `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)`；
5. 失败时清理 temp，并保留 primary/cleanup error 结构。

## 7. Dataset overview 的 unavailable 值

`SizeShape` 中以下字段是 nullable：

- `estimatedDataframeMemoryBytes`
- `duplicatedRows`

Loaded DataFrame 可以计算这些指标。DuckDB-backed table 刻意不做整表 memory estimate 或 full-row duplicate detection，因此返回 `null` 表示 **unavailable**，而不是用 `0` 伪装“没有占用/没有重复”。

DuckDB overview 仍准确提供：

- row/column counts；
- numeric/categorical/string/datetime/bool column counts；
- total nulls、null ratio、columns with nulls、rows with nulls。

## 8. Module map

| File | Responsibility |
|---|---|
| `database_instance.rs` | State-dependent query/edit/export interface |
| `duckdb_reader.rs` | Ingest、Arrow bridge、table metadata |
| `duckdb_editing.rs` | Incremental SQL edit/undo helpers |
| `duckdb_column_snapshot.rs` | Bounded reversible delete-column snapshot |
| `duckdb_analytics.rs` | SQL stats/distribution/overview |
| `duckdb_sql.rs` | Identifier/literal quoting 与 dtype allowlist |
| `edit_operation.rs` | EditOperation、EditHistory、checked JSON/Polars conversion |
| `export.rs` | DuckDB COPY 与 Loaded DataFrame serializers |
| `dataset_overview.rs` | Typed overview DTO 与 in-memory calculation |

验证命令以 [`docs/development/LOCAL_WORKFLOW.md`](../../../docs/development/LOCAL_WORKFLOW.md) 为准，从 repository root 通过 `pnpm` scripts 运行。
