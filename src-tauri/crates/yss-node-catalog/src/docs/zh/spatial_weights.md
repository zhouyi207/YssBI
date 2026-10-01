# 空间权重构造

由地区标识和二维平面坐标生成可连接到 Moran、空间回归或空间面板节点的权重设计对象。

## 输入与参数

`units` 为非空、唯一地区标识；`x`、`y` 为同一行域下的有限数值坐标。至少两个地区，不自动删除缺失值。数字与文本标识区分，整数不转换为浮点。面板数据应先提供每个地区一行的坐标表，再将权重对象连接到面板模型。

- `spatial_weight_rule` 默认 `knn`，还可选 `distance_band` 或 `inverse_distance`。
- `spatial_neighbors` 默认 4，K 近邻时要求 $1\le k<n$。等距时按输入地区顺序打破并列，可存在有向邻接。
- `spatial_radius` 默认 1，必须是正数；距离阈值及反距离规则包含距离等于阈值的邻居。
- `spatial_power` 默认 1，必须是正数；反距离使用 $d_{ij}^{-p}$，阈值内坐标重合时拒绝计算。
- `spatial_symmetrize` 默认关闭；开启后取双向邻接并集，即 $w_{ij}\leftarrow\max(w_{ij},w_{ji})$。
- `spatial_row_standardize` 默认开启；对称化之后，将每个非零行除以行和。行标准化后数值矩阵不一定仍然对称。

## 计算与输出

欧氏距离为 $d_{ij}=\sqrt{(x_i-x_j)^2+(y_i-y_j)^2}$。K 近邻和距离阈值产生二元邻接，反距离产生距离衰减权重；对角线始终为零。至少需要一条有效邻接；孤立地区保留零行。

`result` 包含 `units`、`matrix`、`options` 和 `islands`。`matrix[i][j]` 表示地区 $i$ 的空间滞后中使用地区 $j$ 的权重；`units` 是原输入顺序，`islands` 是从零开始的地区索引。下游通过标识显式匹配，排序变化不会改变地区对应关系。普通截面必须与权重覆盖同一组地区；面板每期都须覆盖全部地区。

这是设计对象，不是显著性检验，不产生 p 值。距离以坐标单位计，输入经纬度不会自动转换为球面距离；需要先转换为适当平面坐标。本节点不读取多边形拓扑，也不构造 Queen/Rook 邻接。密集权重矩阵和序列化结果均计入执行预算。

[近邻与标识顺序](https://pysal.org/libpysal/stable/generated/libpysal.weights.KNN.html) · [权重变换](https://pysal.org/libpysal/stable/generated/libpysal.weights.W.html)
