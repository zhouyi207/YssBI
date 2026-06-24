# Database 模块说明

项目内 tabular 数据统一落在 `database/project.duckdb`（DuckDB）。读写主路径在 `duckdb_reader.rs`：

- **写**：Polars `DataFrame` → Arrow RecordBatch → DuckDB Appender（`appender-arrow`）
- **读**：DuckDB `query_arrow` → Arrow → Polars `DataFrame`

## Categorical / ENUM 类型映射

DataView 中将列 cast 为 **Categorical** 后保存，会写入 DuckDB **ENUM** 类型（内部类型名 `_yssbi_enum_{table}_{column}`）。重开项目时 schema 与数据恢复为 Polars Categorical，不再降级为 String。

### 读写不对称（重要）

| 阶段 | DuckDB 逻辑类型 | Arrow 物理类型 | Polars 逻辑类型 |
|------|-----------------|----------------|-----------------|
| 磁盘 / SQL | `ENUM('a','b',…)` 或 `_yssbi_enum_*` | — | — |
| **读** `query_arrow` | ENUM | **`Dictionary(UInt8, Utf8)`** | 经 `restore_categorical_columns` → `Enum` / Categorical |
| **写** Appender | ENUM 列 | 必须 **`Utf8`**（类别字面量） | Categorical 先 **cast → String** 再 append |

读侧 ENUM 以 Arrow Dictionary 导出；写侧 Appender **不接受** Arrow Dictionary 直写 ENUM 列（包括 `Dictionary(UInt8, Utf8)` 与 Polars 导出的 `Dictionary(UInt32, Utf8View)`），会返回 `Append error`。

因此生产写入在 `polars_series_to_arrow_array(..., for_enum_ingest: true)` 中**强制 String → Utf8 桥接**，而不是 Dictionary roundtrip。

### 写入流程（Categorical 列）

1. `plan_enum_columns`：从 Polars Categorical/Enum 提取 categories
2. `CREATE TYPE "_yssbi_enum_{table}_{col}" AS ENUM (...)` 
3. `CREATE TABLE ... (col "_yssbi_enum_...")`
4. `polars_series_to_arrow_array(series, for_enum_ingest: true)` → Utf8
5. `appender.append_record_batch`

### 读取流程（Categorical 列）

1. `query_arrow` → Polars（列可能为 String 或 Dictionary，取决于导入路径）
2. `duckdb_type_to_raw_string`：ENUM / `_yssbi_enum_*` → schema 展示 `"Categorical"`
3. `restore_categorical_columns`：`DESCRIBE` 识别 ENUM，用 `FrozenCategories` 重建 Polars Enum

### Spike 测试

在 `duckdb_reader.rs` 的 `#[cfg(test)]` 中：

```bash
cd src-tauri
cargo test duckdb_enum_query_arrow_schema -- --nocapture   # 读侧 Arrow 类型
cargo test duckdb_enum_appender_write_paths -- --nocapture # 写侧 Appender 路径 A/B/C
cargo test ingest_categorical_enum_roundtrip              # 端到端 roundtrip
```

| 测试 | 结论 |
|------|------|
| `duckdb_enum_query_arrow_schema` | 读 ENUM → `Dictionary(UInt8, Utf8)` |
| `duckdb_enum_appender_write_paths` A | Utf8 → ENUM ✅ |
| `duckdb_enum_appender_write_paths` B/C | Dictionary → ENUM ❌（预期失败） |

升级 bundled DuckDB 后应重跑上述测试；若 Appender 开始支持 Dictionary，可再评估去掉 String 桥接。

## 其它标量类型（当前行为）

`duckdb_type_to_raw_string` 将多种 DuckDB 整型统一映射为 Polars `"Int64"`；DECIMAL/NUMERIC → `"Float64"`。非 Categorical 列写入时由 Arrow dtype 推断 `CREATE TABLE` SQL（见 `arrow_dtype_to_create_table_sql`）。LIST / STRUCT / MAP 等嵌套类型在 ingest 时暂不支持。

完整严格 DuckDB ↔ Arrow ↔ Polars 对照表尚未全部落地；Categorical/ENUM 路径以本文与代码注释为准。
