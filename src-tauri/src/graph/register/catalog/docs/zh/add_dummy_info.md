# Add Dummy Info

为 **Categorical** **DataSeries** 附加哑变量编码元数据，供 **OLS** 等回归节点使用。序列取值不变，编码规则随输出引用一并传递。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataSeries** | 输入 | 待编码的 **Categorical** 序列 |
| **Drop Category** | 输入（可选） | 作为基准省略的类别（参照水平） |
| **Role** | 输入（可选） | `General`、`Individual` 或 `Time` — **OLS** 对该因子的处理方式 |
| **DataSeries** | 输出 | 附带 `DummyInfo` 元数据的同类序列 |

## 用法

在提取分类列（如 **Decompose DataFrame**）之后接入。**Drop Category** 设为参照组（Stata 风格，该组系数为 0）。因子为个体 ID 或时间索引时选择对应 **Role**。将输出接到 **OLS** 等接受分类自变量的 exog pin。
