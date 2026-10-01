# 重复测量方差分析

输入长表形式的数值 **Response**、受试者标识 **Subject** 和 至少一个受试者内 **Factor** 数列。至少两位受试者，每个因素有至少两个观测类别；每位受试者必须在全部因素组合上恰好有一次观测。各列逐行对齐，关系数列共享行域；缺失值、重复单元格及不完整设计报错，不隐式聚合或删行。本节点只分析受试者内因素，不包含受试者间或混合设计。

返回所有主效应和交互。每项的 $H_0$ 是受试者内相关对比均值全为零，$H_1$ 是至少一个非零。使用正交对比将效应与其受试者交互误差分离：

$$F_T=\frac{SS_T/d_T}{SS_{S\times T}/[(s-1)d_T]}\sim F_{d_T,(s-1)d_T}.$$

$s$ 为受试者数，$d_T$ 为效应对比维数，$SS_{S\times T}$ 为该效应的受试者交互误差，须为正。未校正推断要求受试者独立、误差近似正态且满足该效应的球形性。

**球形性校正** 默认 `greenhouse_geisser`，可选 `none`。若 $S_T$ 是效应正交对比的样本协方差矩阵，则

$$\epsilon_{GG}=\frac{[\operatorname{tr}(S_T)]^2}{d_T\operatorname{tr}(S_T^2)},\qquad df_1=\epsilon_{GG}d_T,\quad df_2=\epsilon_{GG}(s-1)d_T.$$

校正保持 F 不变，仅调整参考自由度；$1/d_T\le\epsilon_{GG}\le1$，单维对比无需校正。未计算 Mauchly 检验。

唯一 **Result** 的 `table` 含各项效应/误差平方和、原始自由度和均方、F、`p_value_uncorrected`、`epsilon_greenhouse_geisser`、采用的 `df_numerator`/`df_denominator`、`p_value` 与偏 $\eta^2$；另有受试者数、组合数及原始因素标签。选择 `none` 时 `p_value` 为未校正值；小 p 值支持相应条件效应或交互，不自动提供事后比较。

校正定义参见 [MathWorks 球形性与 epsilon](https://www.mathworks.com/help/stats/compound-symmetry-assumption-and-epsilon-corrections.html)。
