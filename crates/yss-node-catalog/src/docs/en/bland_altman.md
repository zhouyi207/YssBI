# Bland–Altman agreement

Connect paired Numeric measurements from two methods to **X / method A** and **Y / method B**. Require at least two complete aligned observations. Differences use $d_i=X_i-Y_i$; the horizontal coordinate is $(X_i+Y_i)/2$.

$$\text{bias}=\bar d,\qquad LoA=\bar d\pm z_{(1+c)/2}s_d.$$

Here $s_d$ is the sample standard deviation (n-1 denominator), and c is **Agreement-limit coverage**, default 0.95. **Confidence level** independently controls uncertainty intervals, also default 0.95. Both must be strictly between 0 and 1.

The bias interval is $\bar d\pm t_{1-\alpha/2,n-1}s_d/\sqrt n$. Approximate LoA uncertainty uses standard error $s_d\sqrt{1/n+z^2/[2(n-1)]}$. This assumes independent pairs with approximately normal, stable-variance differences; repeated measures or strong proportional bias need an appropriate extended model.

`result` contains bias, SD, limits, their intervals, and points with zero-based observation indices. All statistics use the complete sample. At most 2000 display points are selected systematically over the whole sample, including endpoints; `sampled` identifies this. Display points are data for inspection and chart composition.

The limits quantify differences; acceptability depends on limits specified for the application. A high correlation does not establish agreement.

Reference: [Bland and Altman (1986)](https://pubmed.ncbi.nlm.nih.gov/2868172/).
