# Reported OR / HR effect sizes

Connect ratio, lower and upper for reported ORs or HRs, with $0<L<R<U$. source*confidence_level is the reported interval level (default 0.95; strictly between 0 and 1). The effect is $y=\log R$; the SE is reconstructed as $s=(\log U-\log L)/(2z*{(1+c)/2})$, and $v=s^2$.
This is a log-Wald interval approximation. Do not mix ORs with HRs, incompatible adjustment sets, outcomes or follow-up definitions. The output confidence_level is separate from the source interval level. Asymmetric profile or bootstrap intervals may not justify this reconstruction.

confidence*level defaults to 0.95 and must be strictly between 0 and 1. result reports measure, studies and confidence_level. studies is a reusable table with study (one-based input row), effect, variance, standard_error, lower and upper; limits are $y\pm z*{(1+c)/2}\sqrt v$ on the selected scale. These are study estimates, without a pooled significance test.

All connected series must be row-aligned and finite. Missing values are rejected; clean the common study table first. Effects must share an analysis scale and contrast direction. New study tables retain positions as explicit keys; join moderator data by a study key before meta-regression.
