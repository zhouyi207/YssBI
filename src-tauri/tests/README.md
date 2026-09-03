# 集成测试文档

本目录包含 YssBI 项目的数据库与科学计算集成测试。

## 如何运行测试

### 运行所有测试

```sh
pnpm test:rs
```

### 运行特定测试

```sh
pnpm test:rs --test database_test
```

### 显示测试输出

默认情况下，Rust 会隐藏测试中的 `println!` 输出。使用 `--nocapture` 参数可以查看完整的执行日志：

```sh
pnpm test:rs --test database_test -- --nocapture
```

## 目录约定

- `common/`：集成测试共享 helper、mock、emitter、executor builder；不放测试数据。
- `data/`：跨领域复用的原始测试数据集，例如 `iris.csv`。
- `sci/fixtures/`：科学计算 golden fixture，包含 input、expected output、tolerance 与 golden source。
- `*.rs`：Cargo 自动识别的 integration test crate；文件名用领域路径命名，例如 `sci_api_time_series_acf_pacf_golden_test.rs`。

## 数据库测试用例

### test_read_iris_csv

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

## Database runtime 使用示例

Database 的 persisted contract 与 session runtime 已有独立 owner；集成测试应直接依赖
`yss-database-contract`、`yss-database-edit` 与 `yss-database-runtime`，不经过根 crate facade。

```rust
use std::sync::Arc;

use yss_database_contract::{DatabaseDecl, DatabaseEngine, DatabaseId};
use yss_database_edit::EditHistory;
use yss_database_runtime::{DatabaseInstance, DatabaseState};

let dataframe = polars::df!("sepal_length" => &[5.1_f64, 6.2]).unwrap();
let mut database = DatabaseInstance {
    decl: DatabaseDecl {
        id: DatabaseId::from_existing("iris_dataset".into()),
        engine: DatabaseEngine::InMemory {
            name: "Iris".into(),
        },
        schema_version: 1,
        required: true,
        name: "Iris".into(),
    },
    state: DatabaseState::Loaded {
        dataframe: Arc::new(dataframe.clone()),
        original: Arc::new(dataframe),
        history: EditHistory::new(),
    },
};

let schema = database.data_schema().unwrap();
assert_eq!(schema.columns().len(), 1);
```

需要验证 session 一致性、分页、catalog snapshot 或 mutation handoff 时，应通过
`DatabaseRuntimeRegistry`、`DatabaseRuntimeSession` 与 `session_api` 构造真实 runtime；完整示例见
[`database_test.rs`](./database_test.rs) 以及 crate 的
[`README.md`](../crates/yss-database-runtime/README.md)。
