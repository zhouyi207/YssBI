# Probit Predict

使用已拟合 **ProbitModel** 对新数据预测。

## 输入

- **Model**（来自 **Probit**）
- 与估计阶段一致的动态 exog pin

## 输出

**Probability** — $P(y=1) = \Phi(x'\hat\beta)$，`DataSeries<Float64>`。

先连接 **Model** 以显示输入 pin。
