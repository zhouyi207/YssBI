# Dagum 基尼系数

将个体数值连接到 **Values**，逐观测分组标签连接到 **Group labels**。两列必须等长、对齐；关系数列须来自同一行域，不能与独立的内存数列混接。标签支持文本、标识符、数值、分类、顺序和二元列，保留原标签；缺失标签会报错。

使用个体等权、未经小样本修正的 Gini，将总体差异分解为

$$G=G_w+G_{nb}+G_t.$$

$G_w$ 是组内差异贡献，$G_{nb}$ 是组间净差异贡献，$G_t$ 是超变密度（组间分布重叠）贡献。设 $p_j=n_j/n$、$\mu_j$ 为组均值、$\mu$ 为总体均值、$s_j=p_j\mu_j/\mu$，则

$$G_w=\sum_jp_js_jG_j,\qquad G_{nb}=\sum_{j>h}\frac{p_jp_h|\mu_j-\mu_h|}{\mu}.$$

组对的交叉平均绝对差记为 $A_{jh}=\frac{1}{n_jn_h}\sum_{i\in j,r\in h}|x_i-x_r|$。组对 Gini 为 $A_{jh}/(\mu_j+\mu_h)$，经济距离为 $D_{jh}=|\mu_j-\mu_h|/A_{jh}$；重叠贡献为 $p_jp_h(A_{jh}-|\mu_j-\mu_h|)/\mu$。

唯一输出 `result` 包含总体 `gini`、`mean`、`observations`，以及 `within`、`between`、`transvariation` 和各自的 `_share` 占比（0–1，非百分数）。`groups` 给出各组观测数、均值、Gini、人口/收入占比及组内贡献；`pairs` 给出组对 Gini、经济距离及两种组间贡献。Inspect 可切换数值与报告视图。

数值必须有限、非负且总体均值为正，缺失值和全零总体会报错。全零子组的 Gini 为 null、组内贡献为 0；两个全零组的组对 Gini 为 null。交叉绝对差为 0 时经济距离为 null；总体 Gini 为 0 时贡献占比为 null。单组时所有差异归于组内。不接收抽样权重，不提供检验或置信区间。

例如数值 `[1, 2, 3, 4]` 分成 `[A, A, B, B]`，得到 `G=0.25`、`within=0.05`、`between=0.20`、`transvariation=0`。两组均值相同仍可能有分布重叠，不能仅由均值差推断总体差异。

方法来源：[Dagum（1997）](https://doi.org/10.1007/BF01205777)。
