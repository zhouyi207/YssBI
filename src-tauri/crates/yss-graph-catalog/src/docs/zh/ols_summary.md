# OLS Summary

与 **OLS** 节点使用相同的输入接口，运行回归后：

1. 输出完整 **OLSResult** 结构体
2. 自动打开 **OLS Summary** 结果窗口（系数、诊断、公式等）

## 模型

$$
Y = X\beta + \varepsilon,\quad \hat{\beta} = (X'X)^{-1}X'Y
$$

残差平方和：

$$
RSS = \sum_{i=1}^{n}(y_i - \hat{y}_i)^2
$$

若只需得到可复用的 **Model** 句柄而不打开窗口，请使用 **OLS** 节点。
