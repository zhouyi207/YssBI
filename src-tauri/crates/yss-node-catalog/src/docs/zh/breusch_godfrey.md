# Breusch–Godfrey 序列相关检验

连接已拟合线性 `model`，检验在控制原回归解释变量后，残差是否存在至指定阶数的序列相关。使用模型保留的残差与原设计，不重新拟合原响应模型；拟合行顺序必须对应所需的时间顺序。

## 参数与假设

| 参数         | 默认值 | 含义                                                                |
| ------------ | ------ | ------------------------------------------------------------------- |
| `lags`       | 1      | 请求阶数 1–40，有效 $h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40)$ |
| `bg_nomiss0` | `true` | 初始缺少的滞后残差填 0，辅助回归保留 $n$ 行；关闭则删除前 $h$ 行    |

**原假设 $H_0$：** $\rho_1=\cdots=\rho_h=0$，控制 $X_t$ 后不存在前 $h$ 阶误差序列相关。

**备择假设 $H_1$：** 至少一个 $\rho_j\ne0$。

## 辅助回归与统计量

对拟合残差 $u_t$ 估计

$$
u_t=X_t'\gamma+\sum_{j=1}^h\rho_j u_{t-j}+v_t.
$$

这里 $X_t'$ 为原设计第 $t$ 行，包含模型已有截距；不会额外加截距。令辅助样本为 $\mathcal I$，$n_{\mathrm{aux}}=|\mathcal I|$，当前使用非中心化决定系数：

$$
R_{\mathrm{aux}}^2
=1-\frac{\sum_{t\in\mathcal I}\hat v_t^2}{\sum_{t\in\mathcal I}u_t^2},
\qquad LM=n_{\mathrm{aux}}R_{\mathrm{aux}}^2,
$$

$$
LM\overset{H_0}{\approx}\chi_h^2,\qquad
p=1-F_{\chi_h^2}(LM).
$$

开启补零时 $n_{\mathrm{aux}}=n$；关闭时为 $n-h$。分母小于等于 $10^{-20}$ 时当前将 $R^2$ 置 0，该退化情况不支持正常推断。辅助回归原理见 [statsmodels Breusch–Godfrey](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.acorr_breusch_godfrey.html)。

## 输出与解释

`result`、`report` 相同，含 `stat=LM`、`p_value`、`lags=h`。只返回 LM 卡方形式，不额外返回辅助 F 检验。

例如 `lags=4` 的 p 值低于预设的 0.05，表示在原解释变量之外，前四阶残差滞后联合有解释作用；不能确定只有第 4 阶存在相关。未拒绝只表示缺乏证据，不证明所有阶数均独立。

经典 BG 需要合适的动态均值设定、误差矩条件及足够样本；可用于含滞后因变量的动态回归，但须满足相应外生性条件。当前即使连接 WLS/GLS，也对**原始残差和原设计作无权重辅助回归**，不自动使用权重、白化变换或稳健协方差；不能据此称为加权或稳健 BG。

至少 4 个观测；删除初始行时另需 $n-h>k+h$（$k$ 为原设计列数）。辅助矩阵奇异、所需计算不可用或非有限结果会失败。节点不识别时间间隔，乱序或跨组拼接会改变检验含义。
