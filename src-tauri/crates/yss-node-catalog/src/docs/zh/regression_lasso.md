# Lasso回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`predictors` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

`lambda` 非负、默认 1。`constant=true`、`standardize=true`：含截距时中心化自变量，再按总体标准差缩放。输出原始单位系数，截距不受惩罚。惩罚能够识别解时支持共线或自变量较多的设计。岭回归直接求解；Lasso 坐标下降默认最多 5000 次迭代、容差 1e-7。`details` 保留目标值及有效自由度（岭回归的迹或 Lasso 非零系数数目）。不自动调参、交叉验证或提供系数 p 值/区间。

$$
\hat\beta=\arg\min_\beta\left\{\frac{RSS}{2n}+\lambda\sum_{j\ne0}|\beta_j|\right\}.
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://scikit-learn.org/stable/modules/generated/sklearn.linear_model.Lasso.html)
