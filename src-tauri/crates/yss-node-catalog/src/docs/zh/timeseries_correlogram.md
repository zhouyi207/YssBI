# 偏（自）相关图：ACF 与 PACF

连接等间隔、按时间排列的有限数值 `series`，至少四条观测且存在非零变异；拒绝缺失值。`ts_maximum_lag=20` 选择 1–40 个显示滞后，实际显示进一步限制为 floor(n/2) − 1。这是滞后显示口径，不是数据行数上限，全部观测均参与相关计算。

$$
\widehat\rho_k=\frac{\sum_{t=k+1}^n(y_t-\bar y)(y_{t-k}-\bar y)}{\sum_{t=1}^n(y_t-\bar y)^2},\qquad
Q(h)=n(n+2)\sum_{k=1}^{h}\frac{\widehat\rho_k^2}{n-k}.
$$

ACF 使用共同的全样本方差分母，PACF 使用 Durbin–Levinson/Yule–Walker 递推。逐点 95% 白噪声参考带为 ±1.96/sqrt(n)，不是同时区间。滞后 h 的 Ljung–Box 检验为 $H_0: \rho_1=\cdots=\rho_h=0$，备择是假定其中至少一个非零，近似服从 h 自由度的卡方分布。节点不扣除已拟合模型的参数自由度；用于模型残差时应另用相应校正检验。

`result` 包含 `acf`、`pacf`、`ciHalfWidth`、`n`。每个绘图项含 `lag`、`value`，ACF 项还含 `qStat`、`pValue`，PACF 项的检验字段为空。结果图同时展示两类相关序列。明显相关可能来自趋势或季节性，仅凭此图不能确定特定 ARIMA 阶数。
