# IV Hausman 内生性检验

连接协方差为 `nonrobust` 的 IV 2SLS `model`，比较同一样本下的 IV 与 OLS 系数。无配置参数；此节点用于 IV 内生性检验，不用于面板 FE/RE 比较。

## 假设与公式

- **原假设 $H_0$：** 待检验解释变量外生，OLS 与 IV 均一致，OLS 有效。
- **备择假设 $H_1$：** 至少一个待检验变量内生，OLS 不一致；有效工具下 IV 仍一致。

$$
\Delta=\hat\beta_{\mathrm{IV}}-\hat\beta_{\mathrm{OLS}},\qquad
A=V_{\mathrm{IV}}-V_{\mathrm{OLS}},
$$

$$
H=\Delta'A^+\Delta,\qquad
q=\operatorname{rank}(A),\qquad
H\overset{H_0}{\approx}\chi_q^2.
$$

$V_{\mathrm{IV}}$、$V_{\mathrm{OLS}}$ 为使用共同 OLS 误差方差计算的协方差（`sigmamore` 口径），$A^+$ 为广义逆。比较包括已有截距；自由度取协方差差矩阵的有效秩。

## 输出与判读

`result`、`report` 相同，含 `hausman` 对象，其字段为 `stat`、`df`、`p_value`。p 值取卡方右尾；$p<\alpha$ 拒绝外生性。

结论以工具有效、相关且模型可识别为前提；未拒绝不证明外生。稳健/聚类协方差或无法得到有效 OLS 比较时失败。

方法细节：[Hausman 检验](https://www.stata.com/manuals/rhausman.pdf)。
