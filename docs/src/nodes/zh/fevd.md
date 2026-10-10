# VAR 预测误差方差分解

连接 VAR Fit 的 `model`，分解各正交创新对预测误差方差的贡献比例。`steps` 范围 1–1000，默认 8。

## 计算公式

$$
\mathrm{FEVD}_{ij}(h)=
\frac{\sum_{s=0}^{h-1}(\Theta_s)_{ij}^2}
{\sum_{s=0}^{h-1}\sum_{j=1}^{K}(\Theta_s)_{ij}^2}.
$$

$\Theta_s$ 为 VAR 的第 $s$ 期正交脉冲响应矩阵，$K$ 为变量数，$h$ 为预测步数。结果表示冲击 $j$ 对变量 $i$ 的预测误差方差贡献；各冲击的比例通常合计为 1。

## 输出与判读

`result` 为结构化结果，`fevd[s][i][j]` 依次表示**累计索引、响应变量、冲击变量**，维度为 `(steps+1) × K × K`。

**数组索引 $s$ 对应预测步数 $h=s+1$**：`fevd[0]` 是一步预测误差分解，`fevd[steps]` 对应 `steps+1` 步。数值为比例，0.25 表示 25%；正负响应方向应查看 IRF。

正交化使用拟合变量顺序的 Cholesky 分解，结果依赖该顺序，创新协方差须正定。节点不提供置信区间或 p 值，也不自动验证模型稳定性。

方法细节：[VAR 方差分解](https://www.statsmodels.org/stable/vector_ar.html#forecast-error-variance-decomposition-fevd)。
