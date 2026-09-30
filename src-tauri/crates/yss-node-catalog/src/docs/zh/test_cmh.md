# Cochran–Mantel–Haenszel 分层检验

在控制分层变量后，检验二元暴露与二元结果之间的共同关联。

## 输入与参数

`exposed`、`outcome` 使用 `0/1` 编码或布尔二元值；`strata` 给出每个对象的层标识，支持数值、分类、有序或二元语义。三列须非空、等长、逐行对应且无空值；关系数列须共享行域。无配置参数。

## 假设与统计量

$H_0$：暴露与结果在各层内条件独立；$H_1$：存在共同方向的层内关联。第 $h$ 层的 $2\times2$ 表为 $\begin{pmatrix}a_h&b_h\\c_h&d_h\end{pmatrix}$，其中 $a_h$ 为暴露和结果均为 1 的人数。令 $n_h$ 为该层总数，$r_h=a_h+b_h$、$c_h^{+}=a_h+c_h$：

$$
E_h=\frac{r_h c_h^{+}}{n_h},\qquad
V_h=\frac{r_h(n_h-r_h)c_h^{+}(n_h-c_h^{+})}{n_h^2(n_h-1)},
$$

$$
X^2_{\mathrm{CMH}}=\frac{[\sum_h(a_h-E_h)]^2}{\sum_h V_h}
\overset{H_0}{\approx}\chi^2_1.
$$

不作连续性校正。单人层不提供统计信息，合计方差必须大于零。观测应独立，关联方向应在层间具有可比性；本检验不检验层间效应异质性。

## 输出与判读

`result` 为结构化结果，`statistic` 为 CMH 卡方统计量，`degrees_of_freedom` 为 `[1]`，`p_value` 为右尾概率，`sample_sizes` 为总观测数。`estimate`、`standard_error` 为空，不输出共同优势比估计。

$p<\alpha$ 支持控制分层后的关联；它不自动排除其他混杂因素。
