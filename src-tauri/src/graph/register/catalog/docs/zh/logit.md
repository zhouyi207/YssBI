# Logit

二元 Logit 回归（IRLS 估计）。对 $y_i \in \{0,1\}$：

$$
P(y_i=1 \mid x_i) = \Lambda(x_i'\beta) = \frac{1}{1+e^{-x_i'\beta}}
$$

## 输入

- **Y** — 二元因变量（`Float64` / `Int64` / `Boolean` 的 `DataSeries`）
- **X** — 一个或多个自变量（`Float64` 或 `Categorical`）
- 可选 **Config**（**Logit Configure**）
- 可选 **Time**（元数据）

## 输出

- **Model** — **LogitModel**，供 **Logit Predict** 使用
- **Fitted** — 样本内预测概率
- **Residuals** — 响应减拟合值

完整报告窗口请用 **Logit Summary**。
