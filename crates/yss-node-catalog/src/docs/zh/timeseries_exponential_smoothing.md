# 指数平滑

本节点为 SES / ETS(A,N,N)，不包含趋势或季节项。

## 输入与设置

连接按时间排列、等间隔、无缺失的有限数值 `series`，不设固定行数上限。`ts_alpha=0.2` 是范围为 [0,1] 的水平系数。`ts_optimize=true` 通过条件残差平方和最小化估计启用的平滑系数；关闭后直接使用指定系数。启用优化时，指定值作为初值，端点会向可行域内部微移。

`ts_horizon=10` 为正整数；`max_iterations=500` 为正整数；`tolerance=0.000001` 范围为 $[10^{-12},0.01]$。无季节时，首条观测初始化水平，启用趋势时用首个差分初始化趋势。季节模型至少需要两个完整周期：两个周期均值之差除以周期长度初始化每期趋势，将首周期均值调整至周期末时点来初始化水平，再以偏差或比率初始化季节状态；估计平滑系数时这些初始状态保持固定。至少需要两条观测；启用优化时，实际拟合行数须大于待估平滑系数数。

## 递推与输出

$$
\ell_t=\alpha y_t+(1-\alpha)\ell_{t-1},\qquad\widehat y_{n+h}=\ell_n.
$$

$\ell$ 为水平，$b$ 为趋势，$s$ 为季节状态。

`parameters` 给出启用的平滑系数。`fitted`、`residuals` 保持原始行位置，首条观测或首个季节周期为 null。`forecasts[0]` 是样本后一期预测；`innovation_variance` 为实际拟合行的均方误差。`log_likelihood`、`aic`、`bic` 使用固定初始化下的条件高斯加性误差，参数数包括估计的平滑系数及方差；完全拟合时这三项为空。不要跨不同初始化样本比较信息准则。`iterations` 记录优化迭代数；不提供预测区间或系数显著性检验。初始化使用样本数据，因此拟合误差并非滚动起点回测结果。

[指数平滑模型](https://www.statsmodels.org/stable/generated/statsmodels.tsa.exponential_smoothing.ets.ETSModel.html)
