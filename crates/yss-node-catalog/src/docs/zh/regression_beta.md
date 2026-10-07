# Beta回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`X₁, X₂, …` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

连续响应须严格在 0 与 1 之间，拒绝端点而不会自动平移。采用似然同时估计均值系数和常数精度。`constant=true`；500 次迭代、容差 1e-7。最后系数 `precision` 保留正的原始尺度并转换 Delta 协方差；拟合值是条件均值，推断采用观测信息和标准正态参考。当前无精度自变量或零/一膨胀 Beta 变体。

$$
Y_i\sim\operatorname{Beta}(\mu_i\phi,(1-\mu_i)\phi),\quad\operatorname{logit}\mu_i=x_i^T\beta,\quad\phi>0.
$$

正的离散度/尺度参数行保留 Delta 标准误及从对数尺度转换的 95% 区间；边界零假设的统计量/p 值为 null。

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/generated/statsmodels.othermod.betareg.BetaModel.html)
