# Cameron–Trivedi 信息矩阵检验分解

连接 OLS 或 WLS 的 `model`，分别报告异方差、偏度、峰度和合计检验。无配置参数；不支持 GLS，原模型应含截距及至少一个解释变量。

以下 $n$ 为样本数，$X$ 为原设计，$u_i$ 为残差，$s^2=\sum_i u_i^2/n$。

## 异方差分量

- **原假设 $H_0$：** 同方差，平方残差辅助方程的非常数系数全为零。
- **备择假设 $H_1$：** 至少一个系数非零。

使用 White 辅助回归：将 $u_i^2$ 对原解释变量、平方项和交叉项组成的 $Z$ 回归。

$$
H=nR_{\mathrm{aux}}^2,\qquad d_H=\operatorname{rank}(Z)-1.
$$

## 偏度分量

- **原假设 $H_0$：** 三阶误差矩限制 $E[X_i(u_i^3-3\sigma^2u_i)]=0$ 成立。
- **备择假设 $H_1$：** 至少一个上述限制不成立。

将 $s_i=u_i^3-3s^2u_i$ 对 $X$ 回归，拟合值记为 $\hat s_i$：

$$
S_{\mathrm{IM}}=n\left(1-\frac{\sum_i(s_i-\hat s_i)^2}{\sum_i s_i^2}\right),
\qquad d_S=\operatorname{rank}(X)-1.
$$

## 峰度分量

- **原假设 $H_0$：** 四阶误差矩限制 $E[u_i^4-6\sigma^2u_i^2+3\sigma^4]=0$ 成立。
- **备择假设 $H_1$：** 该限制不成立。

令 $k_i=u_i^4-6s^2u_i^2+3(s^2)^2$，仅对常数回归：

$$
K_{\mathrm{IM}}=n\left(1-\frac{\sum_i(k_i-\bar k)^2}{\sum_i k_i^2}\right),
\qquad d_K=1.
$$

## 合计检验

- **原假设 $H_0$：** 上述三个分量的矩限制同时成立。
- **备择假设 $H_1$：** 至少一个分量的限制不成立。

$$
IM=H+S_{\mathrm{IM}}+K_{\mathrm{IM}},\qquad d=d_H+d_S+1.
$$

各项均使用其自由度对应的渐近卡方右尾 p 值；$p<\alpha$ 时拒绝该项原假设。自由度以上述节点口径为准。

## 输出与使用说明

`result` 为结构化结果，`test=information_matrix`；内层 `result` 的 `heteroskedasticity`、`skewness`、`kurtosis`、`total` 均含 `chi2`、`df`、`p_value`。

WLS 仅将异方差分量改为对 $w_i u_i^2$ 的 White 检验，偏度与峰度仍使用未加权原始残差。样本须满足 White 完整展开的要求；分量用于定位问题，不作多重检验校正。

方法细节：[Stata IM 分解](https://www.stata.com/manuals/rregresspostestimation.pdf)。
