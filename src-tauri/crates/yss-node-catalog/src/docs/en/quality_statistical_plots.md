# Statistical plots (distribution overview)

Connect one finite, nonempty numeric **values** series. The node shares a single
materialization across three existing plot algorithms, including constant data.
Missing and nonfinite values are rejected.

- **result**: histogram. **bins** = 0 selects Sturges' rule automatically;
  otherwise choose 1–128 display bins. Counts use every observation.
- **ecdf**: empirical cumulative distribution using all observations, with tied
  values combined. Plot metadata reports any display sampling.
- **boxplot**: median, type-7 quartiles, 1.5-IQR whiskers and outliers; all
  observations determine the summary and total outlier count.

The existing plot display budget is 2048 ECDF points or boxplot outlier marks;
larger data are still fully included in the statistics. A visual summary does
not replace a formal distributional test. Use the separate plot nodes when
different inputs or independently configured plots are needed.
