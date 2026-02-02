# T-Test 节点实现完成

## 实现内容

### 1. T-Test 节点 (`src-tauri/src/executor/node/stat/t_test.rs`)
- **输入**:
  - `In` (exec): 执行输入
  - `Sample1` (series): 第一个样本数据
  - `Sample2` (series): 第二个样本数据
- **输出**:
  - `Out` (exec): 执行输出
  - `T` (float64): t 统计量
  - `P` (float64): p 值（双尾检验）
- **功能**: 执行独立样本 t 检验，计算 t 统计量和 p 值
- **日志**: 结果会发送到日志窗口

### 2. Get DataFrame 节点 (`src-tauri/src/executor/node/catalog/data.rs`)
- **输入**:
  - `Name` (string): DataFrame 名称（可选）
- **输出**:
  - `DataFrame` (dataframe): 返回的 DataFrame
- **功能**: 返回示例 Iris 数据集（15 行，包含 setosa、versicolor、virginica 三个品种）
- **数据格式**: JSON 对象数组
  ```json
  [
    {"sepal_length": 5.1, "sepal_width": 3.5, "petal_length": 1.4, "petal_width": 0.2, "species": "setosa"},
    ...
  ]
  ```

### 3. Get Column 节点 (`src-tauri/src/executor/node/catalog/data.rs`)
- **输入**:
  - `DataFrame` (dataframe): 输入的 DataFrame
- **输出**:
  - `Column` (series): 提取的列数据
- **功能**: 从 DataFrame 中提取指定列，列名从节点标题中提取
- **标题格式**: "Get column_name"（例如 "Get sepal_length"）
- **输出格式**: JSON 数组 `[5.1, 4.9, 4.7, ...]`

## 数据流

```
Get DataFrame (Iris 数据集)
    ↓ DataFrame (JSON 对象数组)
    ├─→ Get Column (sepal_length)
    │       ↓ Series (JSON 数组)
    │       → T-Test (Sample1)
    │
    └─→ Get Column (sepal_width)
            ↓ Series (JSON 数组)
            → T-Test (Sample2)
```

## 测试步骤

1. **创建节点**:
   - 添加 `Get DataFrame` 节点（标题可以是 "Get df_xxx"）
   - 添加两个 `Get Column` 节点:
     - 第一个标题设为 "Get sepal_length"
     - 第二个标题设为 "Get sepal_width"
   - 添加 `T-Test` 节点
   - 添加 `Event On Run` 节点

2. **连接节点**:
   ```
   Event On Run (Exec) → T-Test (In)
   Get DataFrame (DataFrame) → Get Column 1 (DataFrame)
   Get DataFrame (DataFrame) → Get Column 2 (DataFrame)
   Get Column 1 (Column) → T-Test (Sample1)
   Get Column 2 (Column) → T-Test (Sample2)
   ```

3. **执行图**:
   - 点击执行按钮
   - 查看日志窗口，应该看到:
     ```
     [Get DataFrame] Loading DataFrame: df_xxx
     [Get DataFrame] Loaded 15 rows
     [Get Column] Extracting column 'sepal_length' from DataFrame
     [Get Column] Extracted 15 values from column 'sepal_length'
     [Get Column] Extracting column 'sepal_width' from DataFrame
     [Get Column] Extracted 15 values from column 'sepal_width'
     [T-Test] Sample1: n=15, mean=5.7467, Sample2: n=15, mean=3.0533
     [T-Test] t = 6.xxxx, p = 0.xxxx
     T-Test: t=6.xxxx, p=0.xxxx (n1=15, n2=15)
     ```

## 预期结果

对于 Iris 数据集的 sepal_length 和 sepal_width:
- **Sample1 (sepal_length)**: n=15, mean≈5.75
- **Sample2 (sepal_width)**: n=15, mean≈3.05
- **t 统计量**: 应该是正值（约 6-7）
- **p 值**: 应该很小（< 0.001），表示两组数据有显著差异

## 技术细节

### 数据转换流程
1. **Get DataFrame**: 返回 JSON 对象数组
2. **Get Column**: 
   - 使用 `json_to_dataframe()` 将 JSON 转换为 Polars DataFrame
   - 使用 `dataframe.column()` 提取列
   - 将 Column 转换为 JSON 数组
3. **T-Test**:
   - 使用 `extract_float_array()` 将 JSON 数组转换为 `Vec<f64>`
   - 执行统计计算
   - 返回 t 和 p 值

### 执行模型
- **Get DataFrame**: DataFlow（纯数据节点，按需求值）
- **Get Column**: DataFlow（纯数据节点，按需求值）
- **T-Test**: FlowAndData（混合节点，有执行流和数据输出）

## 故障排除

### 错误: "Input is not an array"
- **原因**: Get Column 节点没有返回正确的 JSON 数组
- **检查**: 
  1. Get DataFrame 是否正确连接到 Get Column
  2. Get Column 的标题格式是否正确（"Get column_name"）
  3. 列名是否存在于 DataFrame 中

### 错误: "Column 'xxx' not found"
- **原因**: 列名不存在或拼写错误
- **解决**: 检查 Get Column 节点的标题，确保列名正确（sepal_length, sepal_width, petal_length, petal_width, species）

### 错误: "Missing DataFrame input"
- **原因**: Get Column 节点的 DataFrame 输入未连接
- **解决**: 将 Get DataFrame 的输出连接到 Get Column 的输入

## 下一步

可以扩展的功能：
1. 添加更多统计节点（方差分析、相关性分析等）
2. 支持从文件加载 DataFrame
3. 添加数据可视化节点（散点图、直方图等）
4. 支持数据过滤和转换节点
