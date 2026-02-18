# CSV 加载功能完善 - 分析文档

> **已实现**：使用 `load_database`、`delete_database`、`get_database_rows` 命令，前端传入 `DatabaseEngineDTO`。

## 一、当前状态概览

### 1.1 已有基础设施 ✅

| 层级 | 组件 | 状态 |
|------|------|------|
| **前端** | 菜单导入入口 | ✅ `useMenubar.ts` - 文件选择、调用 `DatabaseService.importCSV` |
| **前端** | DatabaseService | ✅ `databaseService.ts` - 封装 `import_csv`、`get_dataframe_rows` |
| **前端** | DatabaseStore | ✅ 存储 databases，addDatabase(id, record) |
| **前端** | DataViewWindow | ✅ 展示表格、分页加载、columns/rowCount |
| **后端** | DatabaseEngine::Csv | ✅ `database_engine.rs` - LazyCsvReader 已实现 |
| **后端** | DatabaseInstance | ✅ 支持 Lazy → Loaded 状态、preview/execution 访问 |
| **后端** | ProjectState | ✅ 含 `project_store.databases: HashMap<String, DatabaseInstance>` |
| **测试** | database_test.rs | ✅ 已有 iris.csv 读取测试 |

### 1.2 缺失/未实现 ❌

| 组件 | 问题 |
|------|------|
| **import_csv** | 当前为 stub，直接返回 `Ok("")`，未做任何事 |
| **get_dataframe_rows** | 返回 `Value::Null`，未实现 |
| **delete_dataframe** | stub |
| **project_store 与 project_data 同步** | `set_data` 会清空 project_store，加载项目时未将 DatabaseDecl 转为 DatabaseInstance |

---

## 二、数据流梳理

```
用户选择 CSV 文件
    ↓
useMenubar.handleImportData()
    ↓
open() 选文件 → DatabaseService.importCSV(path)
    ↓
invoke("import_csv", { path })
    ↓
[Rust] import_csv(path)  ← 需实现
    ├─ 创建 DatabaseEngine::Csv
    ├─ engine.build_lazy() → LazyFrame
    ├─ 创建 DatabaseInstance { decl, state: Lazy }
    ├─ 生成 id (uuid)
    ├─ 写入 ProjectState.project_store.databases
    ├─ 写入 ProjectState.project_data.databases (DatabaseDecl)
    ├─ 获取 schema: columns, row_count (需 collect 或 lazy count)
    └─ 返回 { id, name, rowCount, columnCount, columns }
    ↓
前端 addDatabase(id, dfData)
    ↓
DataViewWindow 展示 → getDataFrameRows(id, offset, limit)
    ↓
invoke("get_dataframe_rows", { id, offset, limit })
    ↓
[Rust] get_dataframe_rows  ← 需实现
    ├─ 从 project_store.databases 取 DatabaseInstance
    ├─ access(DatabaseAccess::Execution) → 触发 ensure_loaded
    ├─ 切片 [offset..offset+limit]
    └─ 转为 JSON 二维数组返回
```

---

## 三、实现步骤建议

### 阶段 1：实现 import_csv（核心）

**文件**: `src-tauri/src/commands/command_dataframe/mod.rs`

1. **注入 ProjectState**：`import_csv(state: State<ProjectState>, path: String)`
2. **创建引擎与实例**：
   ```rust
   let engine = DatabaseEngine::Csv {
       path: path.clone(),
       delimiter: ',',
       has_header: true,
       infer_schema_length: Some(1000),
   };
   let lazy_frame = engine.build_lazy()?;
   ```
3. **生成 ID**：`let id = format!("df-{}", uuid::Uuid::new_v4());`
4. **获取 schema**：对 LazyFrame 做 `limit(1).collect()` 或 `select([]).collect()` 获取列信息；行数可用 `lazy_frame.select([count(lit(1))]).collect()` 或先 `collect()` 再 `height()`
5. **写入双存储**：
   - `project_store.databases.insert(id, DatabaseInstance { ... })`
   - `project_data.databases.insert(id, DatabaseDecl { ... })`
6. **返回 DTO**：`serde_json::to_value(ImportCsvResult { id, name, rowCount, columnCount, columns })`

**返回结构示例**（与前端 DataViewWindow 期望一致）：
```json
{
  "id": "df-xxx",
  "name": "iris.csv",
  "rowCount": 150,
  "columnCount": 5,
  "columns": [
    { "name": "sepal_length", "type": "Float64" },
    { "name": "species", "type": "Utf8" }
  ]
}
```

### 阶段 2：实现 get_dataframe_rows

**文件**: `src-tauri/src/commands/command_dataframe/mod.rs`

1. **参数对齐**：前端传 `{ id, offset, limit }`，Rust 参数需为 `id`, `offset`, `limit`
2. **实现逻辑**：
   ```rust
   let view = state.access_database(&id, DatabaseAccess::Execution)?;
   let df = &view.dataframe;
   let rows = df.slice(offset as i64, limit.min(df.height() - offset));
   // 转为 Vec<Vec<Value>> 返回
   ```

### 阶段 3：项目持久化与加载

- **保存项目**：`project_data.databases` 已包含 DatabaseDecl（含 path），保存到 JSON 时已有
- **加载项目**：`set_data` 会清空 `project_store`，需在加载后根据 `project_data.databases` 中的 DatabaseDecl 重建 DatabaseInstance 并写入 `project_store`

可选实现方式：
- 在 `ProjectState::set_data` 或 `load_project_to_state` 后，遍历 `project_data.databases`，为每个 DatabaseDecl 构建 LazyFrame 并插入 `project_store`
- 或采用懒加载：首次 `access_database` 时，若 store 中不存在，则从 project_data 构建并插入

### 阶段 4：可选增强

- 支持自定义 delimiter、encoding
- 大文件时用 `lazy.count()` 替代全量 `collect()` 获取行数
- 支持 DataFrameCreated 事件，供多窗口同步

---

## 四、关键类型与依赖

| 类型 | 位置 |
|------|------|
| `ProjectState` | `project::ProjectState` |
| `DatabaseEngine` | `database::DatabaseEngine` |
| `DatabaseInstance` | `database::DatabaseInstance` |
| `DatabaseState::Lazy` | `database::DatabaseState` |
| `DatabaseAccess` | `database::DatabaseAccess` |
| Polars `LazyFrame` | `polars::prelude::*` |

---

## 五、建议实施顺序

1. **实现 import_csv**（含 schema 提取、双存储、返回 DTO）
2. **实现 get_dataframe_rows**（含参数修正、切片、序列化）
3. **验证端到端**：选 CSV → 导入 → DataView 展示 → 分页
4. **项目加载时恢复 DatabaseInstance**（可选，用于项目重开后继续查看）
5. **实现 delete_dataframe**（从 store 和 project_data 中移除）

---

## 六、注意事项

- CSV 路径：用户选择的为绝对路径，保存项目时若需相对路径，需结合 `project_path` 做转换
- 大文件：避免在 import 时全量 `collect()`，优先用 lazy 操作获取 schema 和行数
- 错误处理：文件不存在、格式错误等需返回明确错误信息给前端
