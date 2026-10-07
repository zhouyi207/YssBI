# 正态—半正态随机前沿 SFA

通过极大似然估计截面生产或成本前沿。

## 输入和参数

连接完整且对齐的数值 `Y` 和可选 `X₁, X₂, …`。若要拟合对数生产／成本函数，请事先完成变换；节点不自动取对数。

默认 `constant=true`、`frontier_type=production`、`max_iterations=500`、`tolerance=0.0000001`。选择 `cost` 可反转无效率项符号；迭代次数须为正，容差范围 [1e-12,0.01]。
设计须满秩，样本数须大于回归系数数加两个尺度参数。不识别／近零方差分量、非正定信息矩阵或不收敛会明确失败。

## 模型与推断

$$
Y_i=X_i'\beta+v_i-su_i,\quad
v_i\sim N(0,\sigma_v^2),\quad
u_i\sim |N(0,\sigma_u^2)|,
$$

两种误差成分独立，观测彼此独立。生产前沿取 $s=1$，成本前沿取 $s=-1$。令 $\sigma^2=\sigma_u^2+\sigma_v^2$、$\lambda=\sigma_u/\sigma_v$、$\epsilon_i=Y_i-X_i'\beta$，单个观测的对数似然为：

$$
\ell_i=\log(2/\sigma)+\log\phi(\epsilon_i/\sigma)
+\log\Phi(-s\lambda\epsilon_i/\sigma).
$$

两个正尺度参数与回归系数联合估计。使用完整观测信息矩阵，包括回归与尺度的交叉块，计算系数协方差。检验 $H_0:\beta_j=0$ 对 $H_1:\beta_j\ne0$，采用正态 $z=\hat\beta_j/SE_j$、双侧 p 值和 95% 区间；不将零方差检验当作常规 Wald 检验报告。

## 输出

`frontier` 是 $X\hat\beta$，不是结果的条件均值；`residuals` 为观测结果减前沿。`conditional_inefficiency` 为 $E[u_i\mid\epsilon_i]$，`efficiency` 为 $E[\exp(-u_i)\mid\epsilon_i]$，成本模型下也表示成本效率比率。
后者不同于对预期无效率直接取指数。结果还包括系数、协方差、`sigma_u`、`sigma_v`、对数似然及迭代次数。
观测数组均保持输入行序。当前仅实现同方差半正态无效率，不含面板效应或指数／截断正态变体。参见 [随机前沿方法说明](https://www.stata.com/manuals/rfrontier.pdf)。
