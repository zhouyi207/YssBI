# Breusch–Godfrey 序列相关检验

连接线性 `model`，检验控制原解释变量后残差是否存在序列相关。拟合行顺序应为时间顺序，至少 4 个观测。

| 参数         | 默认值 | 含义                                                                |
| ------------ | ------ | ------------------------------------------------------------------- |
| `lags`       | 1      | 请求阶数 1–40，有效 $h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40)$ |
| `bg_nomiss0` | `true` | 初始缺少的滞后残差补零；关闭则删除前 $h$ 行                         |

## 假设与公式

- **原假设 $H_0$：** $\rho_1=\cdots=\rho_h=0$，前 $h$ 阶误差序列相关系数全为零。
- **备择假设 $H_1$：** 至少一个系数非零。

辅助回归使用原残差 $u_t$ 和原设计行 $X_t'$：

$$
u_t=X_t'\gamma+\sum_{j=1}^h\rho_j u_{t-j}+v_t,
\qquad LM=n_{\mathrm{aux}}R_{\mathrm{aux}}^2
\overset{H_0}{\approx}\chi_h^2.
$$

$R_{\mathrm{aux}}^2$ 使用非中心化分母 $\sum u_t^2$。补零时辅助样本数 $n_{\mathrm{aux}}=n$，删除初始行时为 $n-h$。

## 输出与判读

`result` 为结构化结果，含 `stat=LM`、`p_value`、`lags=h`。p 值取卡方右尾；$p<\alpha$ 拒绝前 $h$ 阶联合无相关。

WLS/GLS 输入仍使用原始残差、原设计的无权重辅助回归，不自动加权或白化。删除初始行时另需 $n-h>k+h$（$k$ 为设计列数），辅助设计奇异时无法计算。

方法细节：[Breusch–Godfrey](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.acorr_breusch_godfrey.html)。
