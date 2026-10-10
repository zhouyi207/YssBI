# EGARCH 模型

连接等间隔、按时间排列的有限数值 `series`，通常使用收益率或已经平稳的残差序列；拒绝缺失值，不设数据行数上限。`constant=true` 估计常数均值，关闭则均值固定为零；不拟合 ARMA 条件均值，也不自动对价格做差分。

`ts_p=1` 为正整数的冲击滞后阶数。 `ts_q=1` 为非负整数的方差滞后阶数。 非对称模型的非对称项与冲击项使用相同阶数。`ts_horizon=10` 为正整数；`max_iterations=500` 为正整数；`tolerance=0.000001` 范围为 $[10^{-12},0.01]$。样本数须超过待估参数数及各滞后阶数，且样本方差为正。

$$
\log h_t=\omega+\sum_{i=1}^p\alpha_i(|z_{t-i}|-\sqrt{2/\pi})+\sum_{i=1}^p\gamma_i z_{t-i}+\sum_{j=1}^q\beta_j\log h_{t-j},\qquad z_t=\varepsilon_t/\sqrt{h_t}.
$$

$h_t$ 为条件方差，$\varepsilon_t=y_t-\mu$ 为残差。对数方差 AR 多项式满足平稳约束，alpha 与 gamma 可正可负；高斯标准化冲击的绝对均值为 sqrt(2/pi)。gamma 为负表示负标准化冲击带来更大的波动响应。

高斯条件最大似然使用全部观测，以除数为 n 的中心化样本方差固定初始化样本前方差。样本前冲击平方取该方差，负冲击平方取其一半；EGARCH 样本前的中心化标准冲击项取零。预测使用高斯模拟，对方差取平均，而非对预期对数方差取指数。`ts_simulations=1000` 为正整数，`ts_seed=42` 为非负整数；第一步在拟合参数条件下为精确预测，后续步存在蒙特卡洛误差，增加模拟次数可降低抽样波动。

`parameters` 给出可选均值、omega 与滞后系数，使用输入尺度。`residuals`、`standardized_residuals`、`conditional_variances` 与每条输入行对应。`forecast_variances[0]` 是下一期方差，不是标准差或价格预测。`log_likelihood`、`aic`、`bic`、`observations`、`iterations` 描述拟合，EGARCH 还记录 `simulations` 与 `seed`。不提供系数标准误、Wald 检验或波动率置信区间；不收敛或递推出现非有限值时计算失败。

[条件波动率模型](https://arch.readthedocs.io/en/latest/univariate/generated/arch.univariate.EGARCH.html)
