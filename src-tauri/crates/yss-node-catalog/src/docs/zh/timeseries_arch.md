# ARCH 模型

连接等间隔、按时间排列的有限数值 `series`，通常使用收益率或已经平稳的残差序列；拒绝缺失值，不设数据行数上限。`constant=true` 估计常数均值，关闭则均值固定为零；不拟合 ARMA 条件均值，也不自动对价格做差分。

`ts_p=1` 为正整数的冲击滞后阶数。 非对称模型的非对称项与冲击项使用相同阶数。`ts_horizon=10` 为正整数；`max_iterations=500` 为正整数；`tolerance=0.000001` 范围为 $[10^{-12},0.01]$。样本数须超过待估参数数及各滞后阶数，且样本方差为正。

$$
h_t=\omega+\sum_{i=1}^p\alpha_i\varepsilon_{t-i}^2.
$$

$h_t$ 为条件方差，$\varepsilon_t=y_t-\mu$ 为残差。omega 为正，冲击与方差系数非负，sum(alpha) 小于 1，对应对称高斯新息下的协方差平稳模型。

高斯条件最大似然使用全部观测，以除数为 n 的中心化样本方差固定初始化样本前方差。样本前冲击平方取该方差，负冲击平方取其一半；EGARCH 样本前的中心化标准冲击项取零。预测采用解析递推，将未来冲击平方替换为其条件期望方差。

`parameters` 给出可选均值、omega 与滞后系数，使用输入尺度。`residuals`、`standardized_residuals`、`conditional_variances` 与每条输入行对应。`forecast_variances[0]` 是下一期方差，不是标准差或价格预测。`log_likelihood`、`aic`、`bic`、`observations`、`iterations` 描述拟合，EGARCH 还记录 `simulations` 与 `seed`。不提供系数标准误、Wald 检验或波动率置信区间；不收敛或递推出现非有限值时计算失败。

[条件波动率模型](https://arch.readthedocs.io/en/latest/univariate/generated/arch.univariate.GARCH.html)
