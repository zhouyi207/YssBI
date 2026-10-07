# PSM 价格敏感度

连接对齐的四列报价：**too_cheap** 太便宜、**cheap** 便宜、
**expensive** 贵、**too_expensive** 太贵。价格必须非负且有限，
同一行依此顺序非递减；不合理或缺失的回答直接报错。

**curves** 保留全部出现过的不同价格、四条比例曲线及 not_cheap、not_expensive。
贵的两条曲线使用 F(x) = P(X ≤ x)，便宜的两条使用 1 − F(x)，
相邻出现价格之间按直线连接，不设置固定价格步长。

**range_definition** 默认 original：接受区间的下界为 too_cheap 与 not_cheap
的交点，上界为 not_expensive 与 too_expensive 的交点。
narrower 则分别使用 too_cheap 与 expensive、cheap 与 too_expensive 的交点。

**result** 还输出无差异价格（cheap 与 expensive）和传统“最优”价格
（too_cheap 与 too_expensive）。每个交点返回上下端点：曲线重合时保留整个区间；
观察范围内无交点时为空，不外推。
这些交点描述价格感知，不代表利润或收入最大的价格。

定义参考：[pricesensitivitymeter](https://max-alletsee.github.io/pricesensitivitymeter/reference/psm_analysis.html)。
