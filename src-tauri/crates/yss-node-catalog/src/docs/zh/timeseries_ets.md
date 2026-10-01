# ETS 指数平滑状态空间模型

本节点支持加性误差、可选加性或阻尼趋势、无季节或加性季节，包括 ETS(A,N,N)、ETS(A,A,N)、ETS(A,Ad,N) 及其加性季节版本；不自动搜索或比较 ETS 形式。

## 输入与设置

连接按时间排列、等间隔、无缺失的有限数值 `series`，不设固定行数上限。`ts_alpha=0.2` 是范围为 [0,1] 的水平系数。`ts_optimize=true` 通过条件残差平方和最小化估计启用的平滑系数；关闭后直接使用指定系数。启用优化时，指定值作为初值，端点会向可行域内部微移。 `ts_trend=true`、`ts_damped=false`，启用阻尼须先启用趋势。`ts_seasonality=none` 可选 `none/additive`；`ts_period=12` 为至少 2 的整数，仅季节模型使用。`ts_beta=0.1`、`ts_gamma=0.1`、`ts_phi=0.98` 分别为趋势、季节及阻尼系数，范围为 [0,1]；季节平滑还要求 alpha + gamma ≤ 1。未启用成分的系数不参与递推。

`ts_horizon=10` 为正整数；`max_iterations=500` 为正整数；`tolerance=0.000001` 范围为 $[10^{-12},0.01]$。无季节时，首条观测初始化水平，启用趋势时用首个差分初始化趋势。季节模型至少需要两个完整周期：两个周期均值之差除以周期长度初始化每期趋势，将首周期均值调整至周期末时点来初始化水平，再以偏差或比率初始化季节状态；估计平滑系数时这些初始状态保持固定。至少需要两条观测；启用优化时，实际拟合行数须大于待估平滑系数数。

## 递推与输出

$$
v_t=\ell_{t-1}+\phi b_{t-1},\quad \ell_t=\alpha(y_t-s_{t-m})+(1-\alpha)v_t,\quad b_t=\beta(\ell_t-\ell_{t-1})+(1-\beta)\phi b_{t-1},\quad s_t=\gamma(y_t-v_t)+(1-\gamma)s_{t-m}.
$$

$\ell$ 为水平，$b$ 为趋势，$s$ 为季节状态。 上式为加性季节递推；无季节时去掉季节项，无趋势时趋势为零，无阻尼时 phi = 1。新息形式的趋势系数为 alpha × beta。预测使用末期水平、阻尼趋势累计和及最后一轮季节状态。

`parameters` 给出启用的平滑系数。`fitted`、`residuals` 保持原始行位置，首条观测或首个季节周期为 null。`forecasts[0]` 是样本后一期预测；`innovation_variance` 为实际拟合行的均方误差。`log_likelihood`、`aic`、`bic` 使用固定初始化下的条件高斯加性误差，参数数包括估计的平滑系数及方差；完全拟合时这三项为空。不要跨不同初始化样本比较信息准则。`iterations` 记录优化迭代数；不提供预测区间或系数显著性检验。初始化使用样本数据，因此拟合误差并非滚动起点回测结果。

[指数平滑模型](https://www.statsmodels.org/stable/generated/statsmodels.tsa.exponential_smoothing.ets.ETSModel.html)
