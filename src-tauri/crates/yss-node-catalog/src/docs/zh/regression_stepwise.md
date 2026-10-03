# 逐步回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`X₁, X₂, …` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

`direction` 为 both（默认）、forward/backward；`criterion` 为 aic（默认）或 bic；`constant=true`。OLS 高斯似然的 k 计入系数和一个方差参数。向前/双向从仅截距开始，无截距时从最佳单自变量开始，向后从全部自变量开始。每步选择严格改善最多的候选并排除秩亏模型。`selection_history` 保留动作、原自变量从 1 开始的位置及准则值，`stages` 保留最终模型。最终 t 检验以选择为条件，不调整搜索影响；无按 p 值进入/移除准则或自动留出验证。

$$
AIC=-2\ell+2k,\quad BIC=-2\ell+k\log n.
$$

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.OLS.html)
