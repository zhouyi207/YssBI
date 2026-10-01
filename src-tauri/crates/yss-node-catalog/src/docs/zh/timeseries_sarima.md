# SARIMA 预测

以条件最小二乘拟合一元 SARIMA，在原始观测尺度上预测。

## 输入与参数

连接有限数值序列 `series`，事先按时间排列并保证等间隔。缺失值会报错，不自动排序或删行；没有固定数据行数上限。

`ts_p=1`、`ts_d=1`、`ts_q=0` 分别为非负整数的 AR、普通差分和 MA 阶数。 季节阶数 `ts_seasonal_p=0`、`ts_seasonal_d=1`、`ts_seasonal_q=1` 为非负整数，`ts_period=12` 为至少 2 的整数。 `constant=true` 估计差分后序列的均值：不差分时为水平均值，一阶差分时为漂移，高阶差分时对应积分后的确定性成分。`ts_horizon=10` 为正整数；`ts_confidence=0.95` 严格介于 0 与 1 之间。`max_iterations=500` 为正整数；`tolerance=0.000001` 的范围为 $[10^{-12},0.01]$。

## 估计方法

$$
\phi(B)\Phi(B^m)(w_t-\mu)=\theta(B)\Theta(B^m)\varepsilon_t,\qquad w_t=(1-B)^d(1-B^m)^D y_t.
$$

$B$ 为滞后算子，$\mu$ 为差分后均值，$\varepsilon_t$ 为新息。AR 多项式使用减号，MA 多项式使用加号；季节因子与普通因子相乘。拟合约束 AR 平稳、MA 可逆。样本前误差设为零，差分后的前若干行按 AR/MA 最大滞后阶数排除出目标函数；剩余观测数须超过待估均值及 AR/MA 系数总数，且新息方差为正。阶数由用户指定，不自动选阶。

## 输出与解读

`parameters` 给出 AR/MA 系数、可选的差分均值及新息方差。`fitted`、`residuals` 与原始行对齐，初始不可估计位置为 null。`forecasts[0]` 表示样本后的第一期；`lower`、`upper` 是依据积分后脉冲权重得到的逐点高斯预测区间，以估计参数为条件，不包含参数估计不确定性。

`effective_observations`、`iterations`、`log_likelihood`、`aic`、`bic`、`innovation_variance` 描述条件拟合。AIC/BIC 将新息方差计为一个参数，仅宜在同一变换及估计样本上比较。本节点不提供系数显著性检验；不收敛时报告失败，不返回部分预测。

[季节 ARIMA 表达式](https://otexts.com/fpp3/seasonal-arima.html)
