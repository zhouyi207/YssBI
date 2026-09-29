# IV Hausman 内生性检验

连接协方差设置为 `nonrobust` 的 IV 2SLS `model`，比较同一样本、同一结构方程下的 IV 与 OLS 系数。无配置参数；复用已拟合 IV 系数，并计算用于比较的 OLS。这里不是面板 FE/RE Hausman 检验，也不返回 Durbin 或 Wu–Hausman 的其他形式。

## 假设

**原假设 $H_0$：** 被视为内生的解释变量实际上外生，OLS 与 IV 均一致，在所需同方差等条件下 OLS 有效。

**备择假设 $H_1$：** 至少一个待检验解释变量内生，OLS 不一致，而在工具有效且模型识别的前提下 IV 仍一致。

工具变量的相关性与外生性是本检验推断的前提。Hausman 不是工具变量有效性或弱工具变量检验。

## 计算公式

令 $X$ 为原结构设计（含模型已有截距），$Z$ 为包含外生解释变量和排除工具的工具设计，$n$ 为样本数、$k$ 为结构设计列数：

$$
P_Z=Z(Z'Z)^{-1}Z',\quad \hat X=P_ZX,\quad
\hat\beta_{\mathrm{OLS}}=(X'X)^{-1}X'y,\quad
s_{\mathrm{OLS}}^2=\frac{\sum_i(y_i-X_i'\hat\beta_{\mathrm{OLS}})^2}{n-k}.
$$

当前采用共同 OLS 误差方差（`sigmamore` 口径）：

$$
V_{\mathrm{IV}}=s_{\mathrm{OLS}}^2(\hat X'\hat X)^{-1},\qquad
V_{\mathrm{OLS}}=s_{\mathrm{OLS}}^2(X'X)^{-1},
$$

$$
\Delta=\hat\beta_{\mathrm{IV}}-\hat\beta_{\mathrm{OLS}},\quad
A=V_{\mathrm{IV}}-V_{\mathrm{OLS}},\quad
H=\Delta'A^+\Delta,\quad q=\operatorname{rank}(A),
$$

$$
H\overset{H_0}{\approx}\chi_q^2,\qquad
p=1-F_{\chi_q^2}(H).
$$

$A^+$ 为 SVD 的 Moore–Penrose 广义逆；比较包含保留的全部结构系数和已有截距，自由度按协方差差矩阵的有效秩返回，不直接硬编码为内生变量数。当前将负的数值统计量截为 0。共同方差及截距比较的背景见 [Stata Hausman 手册](https://www.stata.com/manuals/rhausman.pdf)。

## 输出与判读

`result`、`report` 相同，含 `hausman` 对象，字段为 `stat`、`df`、`p_value`。例如在预设 5% 水平 $p=0.02$，表示 IV 与 OLS 的差异支持拒绝外生性；该结论仍以有效、足够相关的工具及正确设定为条件。

未拒绝不能证明不存在内生性；弱工具或小样本可能使检验力不足。稳健/聚类协方差模型、无法得到 OLS 比较、零有效秩、退化方差或非有限结果会使本节点无法给出有效检验并失败。
