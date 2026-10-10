# Firth惩罚Logit回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`X₁, X₂, …` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

二元响应为 0/1 或 false/true，须两种结果均存在。`constant=true`；500 次迭代、容差 1e-7。Jeffreys 惩罚的逻辑似然在设计满秩时可为分离数据提供有限估计。`details.penalized_log_likelihood` 使用原始自变量单位。推断采用未惩罚 Fisher 信息和正态 Wald 近似，不是剖面惩罚似然。保留普通对数似然，不提供未惩罚 AIC/BIC/LR 检验。

$$
\ell^*(\beta)=\ell(\beta)+\tfrac12\log|X^TWX|,\quad W_{ii}=\pi_i(1-\pi_i).
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://search.r-project.org/CRAN/refmans/logistf/html/logistf.html)
