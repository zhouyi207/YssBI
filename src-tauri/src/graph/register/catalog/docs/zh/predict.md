# Predict（预测）

使用已拟合的 **OLSModel**（来自 **OLS**、**WLS** 或 **GLS**）对新自变量做预测。

## 输入

- **Model** — 上游回归节点的 **OLSModel**（或兼容模型句柄）
- **Exog pins** — 根据模型训练时的自变量动态生成（名称与类型与估计阶段一致）

各 exog pin 须为等长 `DataSeries`；分类变量编码须与拟合时一致。

## 输出

**Predicted** — 拟合值 $\hat y = X_{\mathrm{new}} \hat\beta$ 的 `DataSeries<Float64>`。

先连接 **Model**；接好模型后才会出现对应的输入 pin。
