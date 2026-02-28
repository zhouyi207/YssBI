# 集成测试文档

本目录包含 YssBI 项目的集成测试，用于测试节点图的执行逻辑和各种节点的组合使用。

## 如何运行测试

### 运行所有测试

```cmd
cd src-tauri
cargo test
```

### 运行特定测试

```cmd
cd src-tauri
cargo test test_complex_node_graph
cargo test test_nested_sequence_tree
cargo test --test database_test
```

### 显示测试输出（推荐）

默认情况下，Rust 会隐藏测试中的 `println!` 输出。使用 `--nocapture` 参数可以查看完整的执行日志：

```cmd
cd src-tauri
cargo test test_complex_node_graph -- --nocapture
cargo test test_nested_sequence_tree -- --nocapture
```

### 运行测试并显示详细信息

```cmd
cd src-tauri
cargo test -- --nocapture --test-threads=1
```

参数说明：
- `--nocapture`: 显示所有 println! 输出
- `--test-threads=1`: 单线程运行测试，避免输出混乱

## 测试用例说明

### 1. test_complex_node_graph

**测试目标：** 综合测试 sequence、branch、add、equal、print 节点的组合使用

**图结构：**
```
sequence1 (3 个输出)
  ├─ Step 0 -> sequence2 (3 个输出)
  │            ├─ Step 0 -> print("Sequence2-Step0")
  │            ├─ Step 1 -> print("Sequence2-Step1")
  │            └─ Step 2 -> print("Sequence2-Step2")
  ├─ Step 1 -> branch1 (condition=false)
  │            ├─ True -> print("Branch1-True")
  │            └─ False -> print("Branch1-False")
  └─ Step 2 -> branch2 (condition=add(10,10)==20)
               ├─ True -> print("Branch2-True")
               └─ False -> print("Branch2-False")
```

**测试内容：**
- Sequence 节点的顺序执行
- Branch 节点的条件分支
- Add 节点的数学运算
- Equal 节点的比较运算
- 数据流和执行流的正确传递

**预期输出顺序：**
1. Sequence2-Step0
2. Sequence2-Step1
3. Sequence2-Step2
4. Branch1-False (因为 condition=false)
5. Branch2-True (因为 10+10==20 为 true)

**运行命令：**
```cmd
cargo test test_complex_node_graph -- --nocapture
```

---

### 2. test_nested_sequence_tree

**测试目标：** 测试嵌套的 sequence 树形结构，验证深层次的执行流传递

**图结构：**
```
root_sequence (3 个输出)
  ├─ Step 0 -> seq_1_0 (3 个输出)
  │            ├─ Step 0 -> seq_2_0 (3 个输出)
  │            │            ├─ Step 0 -> print("Position: 1-0, 2-0, Step-0")
  │            │            ├─ Step 1 -> print("Position: 1-0, 2-0, Step-1")
  │            │            └─ Step 2 -> print("Position: 1-0, 2-0, Step-2")
  │            ├─ Step 1 -> seq_2_1 (3 个输出)
  │            │            └─ ... (3 个 print)
  │            └─ Step 2 -> seq_2_2 (3 个输出)
  │                         └─ ... (3 个 print)
  ├─ Step 1 -> seq_1_1 (3 个输出)
  │            ├─ Step 0 -> seq_2_3 (3 个输出) -> 3 个 print
  │            ├─ Step 1 -> seq_2_4 (3 个输出) -> 3 个 print
  │            └─ Step 2 -> seq_2_5 (3 个输出) -> 3 个 print
  └─ Step 2 -> seq_1_2 (3 个输出)
               ├─ Step 0 -> seq_2_6 (3 个输出) -> 3 个 print
               ├─ Step 1 -> seq_2_7 (3 个输出) -> 3 个 print
               └─ Step 2 -> seq_2_8 (3 个输出) -> 3 个 print
```

**测试内容：**
- 三层嵌套的 sequence 节点
- 1 个根节点 -> 3 个第一层节点 -> 9 个第二层节点 -> 27 个 print 节点
- 总共 40 个节点的执行流传递
- 每个 print 节点输出其位置信息

**位置信息格式：**
- `Position: 1-X, 2-Y, Step-Z`
  - `1-X`: 第一层 sequence 的索引 (0-2)
  - `2-Y`: 第二层 sequence 的索引 (0-8)
  - `Step-Z`: 当前步骤索引 (0-2)

**预期输出：**
- 27 条 print 输出，按照树的深度优先顺序执行
- 每条输出显示其在树中的位置

**运行命令：**
```cmd
cargo test test_nested_sequence_tree -- --nocapture
```

---

## 测试代码结构

### 通用辅助函数

```rust
fn create_test_registry() -> Arc<NodeRegistry>
```
创建测试用的节点注册表，注册所有内置节点。

### 测试步骤模式

每个测试通常遵循以下步骤：

1. **创建注册表和图**
   ```rust
   let registry = create_test_registry();
   let graph = Arc::new(GraphData::new("test_name", "Description", registry.clone()));
   ```

2. **创建节点**
   ```rust
   let node = graph.create_node("node.type").expect("Failed to create node");
   ```

3. **设置节点参数**
   ```rust
   graph.set_pin_user_value(pin_id, Some(DataValue::Int32(10)))
       .expect("Failed to set value");
   ```

4. **连接节点**
   ```rust
   graph.connect(source_pin_id, target_pin_id)
       .expect("Failed to connect");
   ```

5. **执行图**
   ```rust
   mod common;
   use yssbi_lib::graph::core::GraphRuntime;
   let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(graph.clone())));
   let mut executor = common::executor_for_test(runtime);
   let result = executor.start(start_node);
   assert!(result.is_ok());
   ```

6. **查看日志**
   ```rust
   for log in executor.logs() {
       println!("{}", log);
   }
   ```

## 可用的节点类型

### 控制流节点
- `flow.sequence` - 顺序执行多个步骤
- `flow.branch` - 条件分支

### 数学节点
- `math.add` - 加法运算

### 逻辑节点
- `logic.equal` - 相等比较

### 调试节点
- `debug.print` - 打印输出

## 数据库测试用例

### 3. test_read_iris_csv

**测试目标：** 测试使用 DatabaseEngine::Csv 读取 iris.csv 文件

**测试内容：**
- 创建 CSV 数据库引擎配置
- 构建 LazyFrame（延迟加载）
- 获取预览数据（前 10 行）
- 使用 Preview 访问模式
- 加载完整数据到内存
- 使用 Execution 访问模式

**数据集信息：**
- 文件路径: `tests/data/iris.csv`
- 行数: 150
- 列数: 5
- 列名: sepal_length, sepal_width, petal_length, petal_width, species

**运行命令：**
```cmd
cargo test test_read_iris_csv -- --nocapture
```

---

### 4. test_iris_data_analysis

**测试目标：** 测试读取 CSV 文件并进行基本数据分析

**测试内容：**
- 使用 LazyFrame 读取 CSV 文件
- 收集完整数据到 DataFrame
- 显示数据集基本信息（形状、列名、数据类型）
- 显示前 5 行数据
- 验证数据结构和列名

**预期输出：**
- 数据集形状: 150 行 x 5 列
- 数据类型: Float64 (4列) + String (1列)
- 前 5 行数据的表格展示

**运行命令：**
```cmd
cargo test test_iris_data_analysis -- --nocapture
```

---

### 5. test_iris_lazy_filtering

**测试目标：** 测试使用 LazyFrame 进行数据过滤

**测试内容：**
- 构建 LazyFrame
- 使用 Lazy API 进行过滤（sepal_length > 6.0）
- 选择特定列（sepal_length, sepal_width, species）
- 收集过滤结果
- 验证过滤条件是否正确应用

**过滤条件：**
- 条件: `sepal_length > 6.0`
- 选择列: sepal_length, sepal_width, species
- 预期结果: 61 行 x 3 列

**运行命令：**
```cmd
cargo test test_iris_lazy_filtering -- --nocapture
```

---

## 数据库模块使用示例

### 基本用法

```rust
use yssbi_lib::database::{
    DatabaseAccess, DatabaseDecl, DatabaseEngine, DatabaseInstance, DatabaseState,
};

// 1. 创建 CSV 引擎
let engine = DatabaseEngine::Csv {
    path: "tests/data/iris.csv".to_string(),
    delimiter: ',',
    has_header: true,
    infer_schema_length: Some(100),
};

// 2. 构建 LazyFrame
let lazy_frame = engine.build_lazy().expect("Failed to build lazy frame");

// 3. 创建数据库实例
let mut db_instance = DatabaseInstance {
    decl: DatabaseDecl {
        id: "iris_dataset".to_string(),
        engine,
        schema_version: 1,
        required: true,
    },
    state: DatabaseState::Lazy { lazy_frame },
};

// 4. 获取预览数据
let preview_df = db_instance.get_preview(10).expect("Failed to get preview");

// 5. 加载完整数据
let full_df = db_instance.ensure_loaded().expect("Failed to load data");
```

### 访问模式

数据库实例支持两种访问模式：

1. **Preview 模式** - 用于 UI 预览，限制返回行数（最多 100 行）
   ```rust
   let preview_view = db_instance.access(DatabaseAccess::Preview)?;
   ```

2. **Execution 模式** - 用于图执行，返回完整数据
   ```rust
   let exec_view = db_instance.access(DatabaseAccess::Execution)?;
   ```

### LazyFrame 操作

使用 Polars LazyFrame API 进行数据操作：

```rust
use polars::prelude::*;

// 过滤
let filtered = lazy_frame.filter(col("sepal_length").gt(6.0));

// 选择列
let selected = lazy_frame.select(&[col("sepal_length"), col("species")]);

// 聚合
let aggregated = lazy_frame.group_by(&[col("species")])
    .agg(&[col("sepal_length").mean()]);

// 收集结果
let result = filtered.collect()?;
```
