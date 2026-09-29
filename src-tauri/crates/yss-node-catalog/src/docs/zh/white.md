# White 异方差检验

连接 OLS 或 WLS 的 `model`，检查误差方差是否随解释变量、平方项及交叉项变化。无配置参数；不支持 GLS，原模型应含截距和至少一个解释变量。

## 假设与公式

- **原假设 $H_0$：** 同方差，平方残差辅助方程的非常数系数全为零。
- **备择假设 $H_1$：** 至少一个系数非零，存在这些项可解释的异方差。

将平方残差对原解释变量及其平方、两两交叉项组成的 $Z$ 回归：

$$
LM=nR_{\mathrm{aux}}^2,\qquad
q=\operatorname{rank}(Z)-1,\qquad
LM\overset{H_0}{\approx}\chi_q^2.
$$

$n$ 为拟合样本数，$R_{\mathrm{aux}}^2$ 为辅助回归的中心化决定系数。自由度使用有效秩，重复项不重复计数。

WLS 将被解释量改为 $w_i u_i^2$，仍对原 $Z$ 作无权重辅助回归；$w_i$ 为原精度权重，$u_i$ 为残差。样本数须大于完整展开后的列数。

## 输出与判读

`result`、`report` 相同，`test=white`，内层 `result` 含 `lm_stat`、`df`、`p_value`。p 值取卡方右尾；$p<\alpha$ 拒绝同方差，未拒绝只表示证据不足。

方法细节：[White 检验](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.het_white.html)。
