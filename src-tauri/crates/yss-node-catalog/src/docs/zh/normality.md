# Jarque–Bera / Omnibus 正态性检验

对同一数列同时计算 Jarque–Bera（JB）和 D’Agostino–Pearson Omnibus（$K^2$）检验，检查其分布是否偏离正态分布。两者都利用偏度与峰度，但采用不同的标准化方式，因此统计量和 p 值可能不同。

## 输入与适用条件

连接 `series` 数值序列，例如线性回归的 `residuals`。节点没有配置参数，两个检验始终一起执行，至少需要 8 个非空、有限观测；不自动删除缺失值。检验针对所连接的数列：连接原始变量时检验该变量，连接残差时才是残差正态性诊断。

参考分布以独立同分布的非退化正态样本为基础。回归残差的结论属于模型诊断；明显的序列相关、异方差、聚类或异常值会影响判读。最低可运行样本数不代表近似已经可靠：JB 使用大样本近似，Omnibus 的峰度变换在小样本（尤其 $n\leq20$）下也应谨慎使用。[SciPy 的 JB 说明](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.jarque_bera.html)和[峰度检验说明](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.kurtosistest.html)分别讨论了这些限制。

## 公共符号与样本矩

设输入为 $x_1,\ldots,x_n$，定义

$$
\bar x=\frac1n\sum_{i=1}^n x_i,\qquad
m_r=\frac1n\sum_{i=1}^n(x_i-\bar x)^r,
\qquad S=\frac{m_3}{m_2^{3/2}},\qquad K=\frac{m_4}{m_2^2}.
$$

$S$ 为样本偏度，$K$ 为 Pearson 峰度，正态总体的对应值为 0 和 3。这里的中心矩使用分母 $n$，不采用 $n-1$ 或无偏偏度修正；超额峰度是 $K-3$。

## Jarque–Bera 检验

**原假设 $H_0$：** 样本来自某个均值、方差未知且方差大于零的正态总体，其总体偏度为 0、Pearson 峰度为 3。

**备择假设 $H_1$：** 总体不服从正态分布；本检验通过偏度或峰度偏离正态值来发现这种偏离。仅有偏度 0 和峰度 3 并不足以保证正态性。

统计量及 p 值为

$$
JB=\frac n6\left[S^2+\frac{(K-3)^2}{4}\right],
\qquad JB\overset{H_0}{\approx}\chi^2_2,
\qquad p_{\mathrm{JB}}=1-F_{\chi^2_2}(JB).
$$

$F_{\chi^2_2}$ 为自由度 2 的卡方分布累积分布函数。偏度项和峰度项都非负，统计量越大，对 $H_0$ 越不利。当前使用上述渐近版本，没有小样本校正。

## D’Agostino–Pearson Omnibus 检验

**原假设 $H_0$：** 样本来自非退化正态总体。

**备择假设 $H_1$：** 总体不服从正态分布，其偏度或峰度呈现与正态样本不相容的偏离。

先分别将偏度和峰度变换为近似标准正态统计量 $Z_S$、$Z_K$，再合并：

$$
K^2=Z_S^2+Z_K^2,\qquad
K^2\overset{H_0}{\approx}\chi^2_2,\qquad
p_{\mathrm{Omnibus}}=1-F_{\chi^2_2}(K^2).
$$

这里 $K^2$ 是 Omnibus 的惯用名称，并非上文样本峰度 $K$ 的平方。两项变换依赖样本量；不能直接用 $S$ 和 $K-3$ 代替。[SciPy normaltest](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.normaltest.html)给出了这一组合检验的定义。

### 偏度变换

偏度分量考察总体偏度为 0 的原假设，双侧备择为不等于 0。计算

$$
Y=S\sqrt{\frac{(n+1)(n+3)}{6(n-2)}},\qquad
B=\frac{3(n^2+27n-70)(n+1)(n+3)}
{(n-2)(n+5)(n+7)(n+9)},
$$

$$
W^2=-1+\sqrt{2(B-1)},\quad
\delta=\left(\tfrac12\ln W^2\right)^{-1/2},\quad
a=\sqrt{\frac{2}{W^2-1}},\quad
Z_S=\delta\ln\left[\frac Ya+\sqrt{1+\left(\frac Ya\right)^2}\right].
$$

当前数值约定与所采用的 SciPy 风格变换一致：当 $Y$ 恰好为 0 时，以 $Y=1$ 代入最后一个式子。因此完全对称的样本也不保证返回的 Omnibus 偏度分量为零；复核数值时需保留这一约定。[偏度检验参考](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.skewtest.html)。

### 峰度变换

峰度分量考察总体 Pearson 峰度为 3 的原假设，双侧备择为不等于 3。先使用正态样本峰度的有限样本均值和方差：

$$
E=\frac{3(n-1)}{n+1},\qquad
V=\frac{24n(n-2)(n-3)}{(n+1)^2(n+3)(n+5)},\qquad
X=\frac{K-E}{\sqrt V}.
$$

再计算 Anscombe–Glynn 变换：

$$
b=\frac{6(n^2-5n+2)}{(n+7)(n+9)}
\sqrt{\frac{6(n+3)(n+5)}{n(n-2)(n-3)}},
\qquad
A=6+\frac8b\left(\frac2b+\sqrt{1+\frac4{b^2}}\right),
$$

$$
D=1+X\sqrt{\frac2{A-4}},\qquad
Z_K=
\frac{1-\frac2{9A}
-\operatorname{sgn}(D)\left(\frac{1-2/A}{|D|}\right)^{1/3}}
{\sqrt{2/(9A)}}.
$$

带符号的立方根保留 $D$ 的符号。极端数值下若 $|D|<10^{-300}$，当前计算返回 $Z_K=X$。$Z_S$、$Z_K$ 是合并统计量的中间量，节点不单独输出这两项的检验结果。

## 输出

`result` 和 `report` 包含同一次计算的相同结果。

| 字段                                       | 含义                                 |
| ------------------------------------------ | ------------------------------------ |
| `skewness`                                 | 上述 $S$，正值表示右偏，负值表示左偏 |
| `kurtosis`                                 | Pearson 峰度 $K$，不是超额峰度       |
| `jarque_bera_stat` / `jarque_bera_p_value` | JB 统计量及其右尾 p 值               |
| `omnibus_stat` / `omnibus_p_value`         | Omnibus $K^2$ 统计量及其右尾 p 值    |

## 判读与示例

预先选定显著性水平 $\alpha$（例如 0.05），分别比较两个 p 值：$p<\alpha$ 时拒绝对应的正态性原假设；$p\geq\alpha$ 时只是证据不足以拒绝，不能证明正态。p 值不是“数据服从正态分布的概率”。

例如，$n=100$、$S=0.6$、$K=4$ 时，$JB\approx10.167$，$p_{\mathrm{JB}}\approx0.0062$，在 5% 水平拒绝 $H_0$。Omnibus 仍需按上述样本量变换单独计算，不能复用 JB 的 p 值。两者使用同一样本，并非相互独立的两份证据；本节点也不合并 p 值或执行多重检验校正。

结合 Q–Q 图、直方图、异常值检查和模型背景判断偏离是否影响分析。小样本可能缺乏检验力，大样本可能对很小的偏离也显著。

常量序列的 $m_2=0$，偏度和峰度在数学上未定义。当前节点会按退化约定使用 $S=0$、$K=3$ 继续计算；这种结果不能解释为“通过正态性检验”。观测不足、空值、非有限输入或非有限计算结果会失败。
