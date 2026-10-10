# Phillips–Perron 单位根检验

连接按等间隔时间顺序排列的有限数值 `series`，至少四条观测且无缺失；不设固定行数上限。`ts_deterministic=constant` 可选 `none`、`constant`、`trend`（截距与线性趋势）；`ts_bandwidth=4` 为非负整数的 Newey–West 截断滞后数，须小于回归观测数 $N=n-1$，不是 ADF 的增广滞后阶数。

$H_0$：序列存在单位根；$H_1$：序列围绕所选确定性项平稳。先将 $y_t$ 对 $y_{t-1}$ 及确定性项回归，再进行 PP tau 校正：

$$
Z_\tau=\sqrt{\frac{\gamma_0}{\lambda^2}}\frac{\widehat\rho-1}{SE(\widehat\rho)}-\frac{\lambda^2-\gamma_0}{2\lambda}\frac{N\,SE(\widehat\rho)}{s}.
$$

其中 $\gamma_0=\sum e_t^2/N$，$s^2=\sum e_t^2/(N-k)$，$\lambda^2$ 为 Bartlett 权重的 Newey–West 长期方差，$k$ 为回归系数数。残差与长期方差须为正，回归设计须满秩且保留残差自由度。

`statistic` 是 PP tau；`p_value` 是所选确定性形式对应的 MacKinnon 响应曲面近似，`p_value_kind=mackinnon_approximation`。参考分布是非标准单位根分布，不是 Student t；较小的左尾 p 值支持拒绝单位根原假设。`observations`、`effective_observations`、`bandwidth`、`deterministic`、`long_run_variance` 记录实际设定；本节点不计算有限样本 PP 临界值，因此 `critical_values` 为空。确定性项应依据研究问题选择，不应按最小 p 值挑选。

[Phillips–Perron 参考](https://arch.readthedocs.io/en/latest/unitroot/generated/arch.unitroot.PhillipsPerron.html)
