# OR / HR 效应量

连接已报告 OR 或 HR 的 ratio、lower、upper，要求 $0<L<R<U$。source*confidence_level 为原区间置信水平，默认 0.95，须严格介于 0 和 1。效应 $y=\log R$；通过 $s=(\log U-\log L)/(2z*{(1+c)/2})$ 重建标准误，方差 $v=s^2$。
此换算采用对数 Wald 区间近似。不要混合 OR 与 HR、不同调整变量集、不同结局或随访定义。输出 confidence_level 与原报告区间水平分别设置。非对称轮廓似然或 Bootstrap 区间可能不适合此重建。

confidence*level 默认 0.95，须严格介于 0 和 1。result 报告 measure、studies、confidence_level。studies 是可继续连接的表，包含 study（从 1 开始的输入行号）、effect、variance、standard_error、lower、upper；区间为所选尺度上的 $y\pm z*{(1+c)/2}\sqrt v$。这些是各研究估计值，不包含合并显著性检验。

所有连接列必须逐行对齐且为有限值。节点拒绝缺失值，请先清理共同的研究表。效应须使用相同分析尺度和对照方向。新研究表以位置字段保留显式键；做 Meta 回归前可按研究键连接协变量。
