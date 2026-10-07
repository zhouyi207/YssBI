# VAR 正交脉冲响应

连接 VAR Fit 的 `model`，计算一个标准差正交创新冲击对各变量的逐期响应。`steps` 范围 1–1000，默认 8，输出第 0 到 `steps` 期。

## 计算公式

$$
\Phi_0=I,\qquad
\Phi_s=\sum_{\ell=1}^{\min(s,p)}A_\ell\Phi_{s-\ell},
\qquad \Sigma=GG',\qquad \Theta_s=\Phi_sG.
$$

$A_\ell$ 为拟合 VAR 的滞后系数矩阵，$p$ 为最大滞后，未纳入的滞后按零处理；$G$ 为创新协方差 $\Sigma$ 的下三角 Cholesky 因子，$\Theta_s$ 为第 $s$ 期正交响应。

## 输出与判读

`result` 为结构化结果，`oirf[s][i][j]` 依次表示**期数、响应变量、冲击变量**，维度为 `(steps+1) × K × K`，$K$ 为变量数。第 0 期为冲击当期，`oirf[0]` 等于 $G$。

响应单位与响应变量相同，为逐期值，不是累计值。变量顺序沿用拟合模型，改变顺序通常改变正交响应；创新协方差必须正定。

节点输出点估计，不提供置信区间或 p 值，也不自动验证 VAR 稳定性。

方法细节：[VAR 脉冲响应](https://www.statsmodels.org/stable/vector_ar.html#impulse-response-analysis)。
