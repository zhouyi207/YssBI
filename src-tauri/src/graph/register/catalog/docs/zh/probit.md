# Probit

二元 Probit 回归（IRLS）。对 $y_i \in \{0,1\}$：

$$
P(y_i=1 \mid x_i) = \Phi(x_i'\beta)
$$

其中 $\Phi$ 为标准正态 CDF。

## 输入

- **Y** — 二元因变量
- **X** — 自变量（`Float64` 或 `Categorical`）
- 可选 **Config**、可选 **Time**

## 输出

- **Model** — **ProbitModel**，供 **Probit Predict**
- **Fitted** / **Residuals**

报告窗口请用 **Probit Summary**。
