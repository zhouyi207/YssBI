# Logit Predict

使用已拟合 **LogitModel** 对新数据预测。

## 输入

- **Model**（来自 **Logit**）
- 动态 exog pin（名称与类型同估计阶段）

## 输出

**Probability** — $P(y=1) = \Lambda(x'\hat\beta)$，`DataSeries<Float64>`。

先连接 **Model** 以显示输入 pin。
