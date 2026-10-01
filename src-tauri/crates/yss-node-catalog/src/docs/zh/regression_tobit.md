# Tobit模型

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`predictors` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

正态 Tobit：`censoring=left`（默认）或 `both`。`lower=0`；`upper=1` 只用于双侧且须大于下界。响应须在界内，恰好在边界的值作为删失；至少一个未删失观测。`constant=true`；500 次迭代、容差 1e-7。似然结合删失正态尾概率及未删失密度。最后系数为正的 `sigma`，采用 Delta 协方差及正态 Wald 推断。拟合值是潜在均值，不是删失后响应期望；处理删失而非截断样本。

$$
Y_i^*=x_i^T\beta+\varepsilon_i,\quad\varepsilon_i\sim N(0,\sigma^2),\quad Y_i=\max(L,Y_i^*)\ \text{or}\ \min(U,\max(L,Y_i^*)).
$$

正的离散度/尺度参数行保留 Delta 标准误及从对数尺度转换的 95% 区间；边界零假设的统计量/p 值为 null。

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/examples/notebooks/generated/generic_mle.html)
