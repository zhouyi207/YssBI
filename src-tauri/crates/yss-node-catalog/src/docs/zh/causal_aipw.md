# 双重稳健估计 AIPW

## 输入与识别

连接对齐的数值 `response`、二元或数值 0/1 的 `treatment`、可选的重复数值端口 `predictors`。
false/true 表示对照／处理，样本中必须同时出现两种状态。协变量须在处理前测量；不连接协变量时拟合仅截距模型。缺失、非有限、完全分离或奇异输入会失败，不静默删行。

ATE 以样本协变量总体为目标，ATT 以处理组协变量总体为目标。因果解释要求一致性、无干扰、给定协变量后的可交换性及重叠；拟合成功并不证明这些假设成立。

## 估计与参数

将 Logit 倾向模型与处理／对照两组的 OLS 结果模型结合；全部模型包含截距并使用同一协变量列表。每个结果子模型都须可识别且有正残差自由度。

令 $D_i\in\{0,1\}$，倾向概率为 $e_i$，条件均值预测为 $\hat m_d(X_i)$，处理组数量为 $n_1$：

$$
\widehat{ATE}=\frac1n\sum_i\left[
\hat m_1(X_i)-\hat m_0(X_i)
+\frac{D_i(Y_i-\hat m_1(X_i))}{e_i}
-\frac{(1-D_i)(Y_i-\hat m_0(X_i))}{1-e_i}\right].
$$

$$
\widehat{ATT}=\frac1{n_1}\sum_i\left[
D_i(Y_i-\hat m_0(X_i))
-\frac{(1-D_i)e_i(Y_i-\hat m_0(X_i))}{1-e_i}\right].
$$

在识别和重叠假设成立时，ATE 的一致性要求倾向模型正确，或两个结果均值模型都正确；ATT 要求倾向模型或对照结果均值模型正确。“双重稳健”不能解决未观测混杂。当前不使用机器学习拟合或交叉拟合。`potential_outcomes` 依次为对照和处理状态预测。

倾向得分是含截距 Logit 模型给出的 $P(D=1\mid X)$。`ps_overlap=0.000001` 须严格介于 0 与 0.5 之间，所有估计得分须位于 `[ps_overlap,1-ps_overlap]`；不裁剪得分或删行。`max_iterations=500` 为正整数，`tolerance=0.0000001` 的范围为 [1e-12,0.01]；不收敛时返回失败。

## 推断

参数 `bootstrap_replications=0`（默认）仅计算点估计；启用推断时须为至少 2 的整数。`seed=42` 为默认非负随机种子，可重现抽样；实际推断通常需要远多于两次重复。
每次对独立观测整行有放回抽样，重新拟合全部倾向／结果阶段。任何一次拟合失败都会终止，不丢弃失败重复；此抽样方案不适用于聚类或时间相关数据。

用 bootstrap 标准差作为 $SE$。检验 $H_0:\theta=0$ 对 $H_1:\theta\ne0$，统计量 $z=\hat\theta/SE$ 采用渐近标准正态分布，给出双侧 p 值与 $\hat\theta\pm1.96SE$ 的 95% 区间。区间为正态近似而非百分位区间；关闭 bootstrap 时全部推断字段为空。

## 输出

`result` 使用处理效应结果类型，包含 `ate`、`att`、样本数量、方法与推断信息。每个效应包含 `estimate` 以及可为空的 `standard_error`、`statistic`、`p_value`、`confidence_interval`。适用的倾向／预测／匹配数组保持原行顺序。可将结果连接至 ATE 或 ATT 节点，提取相应目标量。
