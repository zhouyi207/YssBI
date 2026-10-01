# 零膨胀负二项回归

连接对齐的有限观测，缺失值会被拒绝。除下述说明外，响应为数值，`predictors` 按端口顺序接收 至少一个数值列，对应 x1、x2……。分类自变量须显式编码；未惩罚模型须设计满秩且残差自由度为正。

## 方法与参数

响应为非负整数计数且均值为正，f 为 NB2。计数项使用 `predictors`；可选 `inflation_predictors` 配置零生成 Logit，该子模型始终含截距、默认仅截距。`constant=true` 只控制计数截距。观测数须大于总参数数目；500 次迭代、容差 1e-7。系数顺序：计数项、`inflation.*`，最后为 `alpha`。拟合值为混合均值 (1−π)μ。推断采用含子模型间协方差的完整观测 Hessian 和正态 Wald 参考。局部优化不保证混合似然的全局最优；当前无偏置/暴露量/跨栏变体。

$$
P(Y=0)=\pi+(1-\pi)f(0;\mu),\quad P(Y=y>0)=(1-\pi)f(y;\mu),\quad\log\mu=x^T\beta,\quad\operatorname{logit}\pi=z^T\gamma.
$$

正的离散度/尺度参数行保留 Delta 标准误及从对数尺度转换的 95% 区间；边界零假设的统计量/p 值为 null。

## 输出

唯一输出是结构化 `result`，在 Inspect 中查看数值或报告。模型保留系数、协方差、拟合/残差数组、`statistics`、迭代信息及方法特有 `details`。有定义时，系数推断含标准误、以所报告系数为零作为 H0 的双侧检验及 95% 区间。未定义/不适用推断为 `null`，不适用数组为空；工作流在 `stages` 中保留模型和明确选择信息。不收敛/数值失败会使执行失败。

[方法参考](https://www.statsmodels.org/stable/generated/statsmodels.discrete.count_model.ZeroInflatedNegativeBinomialP.html)
