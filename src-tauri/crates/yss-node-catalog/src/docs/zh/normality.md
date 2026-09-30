# Jarque–Bera / Omnibus 正态性检验

连接 `series` 数值序列（如回归残差），同时计算两种正态性检验。无配置参数；至少需要 8 个无空值、有限观测。

## Jarque–Bera 检验

- **原假设 $H_0$：** 总体服从正态分布。
- **备择假设 $H_1$：** 总体不服从正态分布，通过偏度或峰度的偏离识别。

$$
JB=\frac n6\left[S^2+\frac{(K-3)^2}{4}\right],
\qquad JB\overset{H_0}{\approx}\chi^2_2.
$$

$n$ 为样本数，$S$ 为样本偏度，$K$ 为 Pearson 峰度（正态基准为 3）。节点使用分母为 $n$ 的样本中心矩，不作无偏修正。

## D’Agostino–Pearson Omnibus 检验

- **原假设 $H_0$：** 总体服从正态分布。
- **备择假设 $H_1$：** 总体不服从正态分布，通过偏度或峰度的偏离识别。

$$
K^2=Z_S^2+Z_K^2,\qquad K^2\overset{H_0}{\approx}\chi^2_2.
$$

$Z_S$、$Z_K$ 分别为经过样本量调整的偏度、峰度标准化统计量；这里的 $K^2$ 是检验名称，不是样本峰度的平方。两种方法的标准化不同，p 值可能不同。

## 输出与判读

`result` 为结构化结果：

| 字段                                       | 含义                    |
| ------------------------------------------ | ----------------------- |
| `skewness` / `kurtosis`                    | 样本偏度 / Pearson 峰度 |
| `jarque_bera_stat` / `jarque_bera_p_value` | JB 统计量及 p 值        |
| `omnibus_stat` / `omnibus_p_value`         | Omnibus 统计量及 p 值   |

两种检验均使用自由度 2 的卡方右尾 p 值。$p<\alpha$ 时拒绝对应原假设；否则为证据不足以拒绝。节点不合并两个 p 值。

小样本下渐近近似需谨慎，尤其 Omnibus 的峰度部分在 $n\leq20$ 时。常量序列不适合此检验，即使返回数值也不能解释为通过正态性检验。

方法细节：[Jarque–Bera](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.jarque_bera.html)、[Omnibus](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.normaltest.html)。
