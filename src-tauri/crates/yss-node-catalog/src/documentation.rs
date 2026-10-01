use yss_node_protocol::NodeTypeId;

#[derive(Clone, Copy)]
struct Documentation {
    en: &'static str,
    zh: Option<&'static str>,
}

macro_rules! markdown {
    ($slug:literal) => {
        Documentation {
            en: include_str!(concat!("docs/en/", $slug, ".md")),
            zh: Some(include_str!(concat!("docs/zh/", $slug, ".md"))),
        }
    };
}

pub(crate) fn documentation(node_type_id: &NodeTypeId, locale: &str) -> Option<Box<str>> {
    mapped_documentation(node_type_id.as_str())
        .map(|documentation| select_locale(documentation, locale).into())
        .or_else(|| super::statistics::inventory_documentation(node_type_id.as_str(), locale))
        .or_else(|| super::dataframe::inventory_documentation(node_type_id.as_str(), locale))
        .or_else(|| super::dataframe::aggregation_documentation(node_type_id.as_str(), locale))
        .or_else(|| super::dataframe::transformation_documentation(node_type_id.as_str(), locale))
}

fn mapped_documentation(node_type_id: &str) -> Option<Documentation> {
    Some(match node_type_id {
        "yssbi.statistics.regression.robust" => markdown!("regression_robust"),
        "yssbi.statistics.regression.hierarchical" => markdown!("regression_hierarchical"),
        "yssbi.statistics.regression.stepwise" => markdown!("regression_stepwise"),
        "yssbi.statistics.regression.curve" => markdown!("regression_curve"),
        "yssbi.statistics.regression.nonlinear" => markdown!("regression_nonlinear"),
        "yssbi.statistics.regression.nonlinear_formula" => {
            markdown!("regression_nonlinear_formula")
        }
        "yssbi.statistics.regression.ridge" => markdown!("regression_ridge"),
        "yssbi.statistics.regression.lasso" => markdown!("regression_lasso"),
        "yssbi.statistics.regression.pls" => markdown!("regression_pls"),
        "yssbi.statistics.regression.logit.multinomial" => {
            markdown!("regression_logit_multinomial")
        }
        "yssbi.statistics.regression.logit.ordinal" => markdown!("regression_logit_ordinal"),
        "yssbi.statistics.regression.logit.firth" => markdown!("regression_logit_firth"),
        "yssbi.statistics.regression.poisson" => markdown!("regression_poisson"),
        "yssbi.statistics.regression.negative_binomial" => {
            markdown!("regression_negative_binomial")
        }
        "yssbi.statistics.regression.zero_inflated_poisson" => {
            markdown!("regression_zero_inflated_poisson")
        }
        "yssbi.statistics.regression.zero_inflated_negative_binomial" => {
            markdown!("regression_zero_inflated_negative_binomial")
        }
        "yssbi.statistics.regression.tobit" => markdown!("regression_tobit"),
        "yssbi.statistics.regression.logit.conditional" => {
            markdown!("regression_logit_conditional")
        }
        "yssbi.statistics.regression.deming" => markdown!("regression_deming"),
        "yssbi.statistics.regression.quantile" => markdown!("regression_quantile"),
        "yssbi.statistics.workflow.regression.univariate_multivariable" => {
            markdown!("regression_workflow_univariate_multivariable")
        }
        "yssbi.statistics.workflow.regression.grouped" => markdown!("regression_workflow_grouped"),
        "yssbi.statistics.workflow.regression.baseline" => {
            markdown!("regression_workflow_baseline")
        }
        "yssbi.statistics.regression.threshold" => markdown!("regression_threshold"),
        "yssbi.statistics.transform.rcs" => markdown!("regression_rcs"),
        "yssbi.statistics.regression.glm" => markdown!("regression_glm"),
        "yssbi.statistics.regression.gamma" => markdown!("regression_gamma"),
        "yssbi.statistics.regression.inverse_gaussian" => markdown!("regression_inverse_gaussian"),
        "yssbi.statistics.regression.cloglog" => markdown!("regression_cloglog"),
        "yssbi.statistics.regression.beta" => markdown!("regression_beta"),
        "yssbi.statistics.regression.fractional_response" => {
            markdown!("regression_fractional_response")
        }
        "yssbi.statistics.association.canonical" => markdown!("multivariate_canonical"),
        "yssbi.statistics.multivariate.exploratory_factor" => markdown!("multivariate_factor"),
        "yssbi.statistics.multivariate.pca" => markdown!("multivariate_pca"),
        "yssbi.statistics.multivariate.correspondence" => markdown!("multivariate_correspondence"),
        "yssbi.statistics.multivariate.discriminant" => markdown!("multivariate_discriminant"),
        "yssbi.statistics.multivariate.rda" => markdown!("multivariate_rda"),
        "yssbi.statistics.multivariate.mds" => markdown!("multivariate_mds"),
        "yssbi.statistics.anova.one_way" => markdown!("anova_one_way"),
        "yssbi.statistics.anova.two_way" => markdown!("anova_two_way"),
        "yssbi.statistics.anova.three_way" => markdown!("anova_three_way"),
        "yssbi.statistics.anova.factorial" => markdown!("anova_factorial"),
        "yssbi.statistics.anova.ancova" => markdown!("anova_ancova"),
        "yssbi.statistics.anova.manova" => markdown!("anova_manova"),
        "yssbi.statistics.anova.repeated_measures" => markdown!("anova_repeated_measures"),
        "yssbi.statistics.inequality.theil" => markdown!("theil"),
        "yssbi.statistics.inequality.gini" => markdown!("gini"),
        "yssbi.statistics.inequality.dagum_gini" => markdown!("dagum_gini"),
        "yssbi.statistics.association.pearson" => markdown!("pearson"),
        "yssbi.statistics.association.partial" => markdown!("partial_correlation"),
        "yssbi.statistics.association.spearman" => markdown!("spearman"),
        "yssbi.statistics.association.kendall" => markdown!("kendall"),
        "yssbi.statistics.test.kappa" => markdown!("kappa"),
        "yssbi.statistics.association.icc" => markdown!("icc"),
        "yssbi.statistics.association.bland_altman" => markdown!("bland_altman"),
        "yssbi.statistics.test.kendall_w" => markdown!("kendall_w"),
        "yssbi.statistics.association.ridit" => markdown!("ridit"),
        "yssbi.statistics.association.rwg" => markdown!("rwg"),
        "yssbi.numeric.add" => markdown!("add"),
        "yssbi.numeric.subtract" => markdown!("subtract"),
        "yssbi.numeric.multiply" => markdown!("multiply"),
        "yssbi.numeric.divide" => markdown!("divide"),
        "yssbi.numeric.ln" => markdown!("ln"),
        "yssbi.numeric.log2" => markdown!("log2"),
        "yssbi.numeric.log10" => markdown!("log10"),
        "yssbi.numeric.power" => markdown!("power"),
        "yssbi.numeric.log" => markdown!("log"),
        "yssbi.numeric.sqrt" => markdown!("sqrt"),
        "yssbi.numeric.square" => markdown!("square"),

        "yssbi.logic.equal" => markdown!("equal"),
        "yssbi.logic.less"
        | "yssbi.logic.less_equal"
        | "yssbi.logic.greater"
        | "yssbi.logic.greater_equal" => markdown!("comparison"),
        "yssbi.logic.not_equal" => markdown!("not_equal"),
        "yssbi.logic.and" => markdown!("and"),
        "yssbi.logic.or" => markdown!("or"),
        "yssbi.logic.not" => markdown!("not"),

        "yssbi.value.convert" => markdown!("convert"),
        "yssbi.project.function.call" => markdown!("call_function"),
        "yssbi.constant.get" => markdown!("get_constant"),
        "yssbi.constant.pi" => markdown!("pi"),
        "yssbi.constant.e" => markdown!("e"),
        "yssbi.debug.view" => markdown!("view"),

        "yssbi.dataframe.source.get" => markdown!("get_dataframe"),
        "yssbi.dataframe.project" => markdown!("select_columns"),
        "yssbi.dataframe.drop.columns" => markdown!("drop_columns"),
        "yssbi.dataframe.drop.rows" => markdown!("drop_rows"),
        "yssbi.dataframe.decompose" => markdown!("decompose_dataframe"),
        "yssbi.dataframe.combine" => markdown!("combine_dataframe"),
        "yssbi.dataframe.concat.rows" => markdown!("concatenate_rows"),
        "yssbi.dataframe.concat.columns" => markdown!("concatenate_columns"),
        "yssbi.dataframe.join" => markdown!("join_dataframe"),
        "yssbi.dataframe.series.select" => markdown!("get_dataseries"),
        "yssbi.dataframe.series.length" => markdown!("dataseries_length"),
        "yssbi.dataframe.series.sum" => markdown!("dataseries_sum"),
        "yssbi.dataframe.series.mean" => markdown!("dataseries_mean"),
        "yssbi.dataframe.series.standardize" => markdown!("standardize_dataseries"),
        "yssbi.dataframe.series.inverse_standardize" => {
            markdown!("inverse_standardize_dataseries")
        }
        "yssbi.dataframe.series.annotate_dummy" => markdown!("add_dummy_info"),
        "yssbi.dataframe.timeseries.align" => markdown!("ts_align"),
        "yssbi.dataframe.timeseries.difference" => markdown!("ts_diff"),
        "yssbi.dataframe.timeseries.percent_change" => markdown!("ts_pct_change"),
        "yssbi.dataframe.timeseries.rolling_mean" => markdown!("ts_rolling_mean"),
        "yssbi.dataframe.timeseries.lag" => markdown!("ts_lag"),
        "yssbi.dataframe.panel.align" => markdown!("xt_align"),
        "yssbi.dataframe.panel.difference" => markdown!("xt_diff"),

        "yssbi.distribution.bernoulli.sample" => markdown!("bernoulli"),
        "yssbi.distribution.beta.sample" => markdown!("beta"),
        "yssbi.distribution.binomial.sample" => markdown!("binomial"),
        "yssbi.distribution.cauchy.sample" => markdown!("cauchy"),
        "yssbi.distribution.chi_squared.sample" => markdown!("chi_squared"),
        "yssbi.distribution.discrete_uniform.sample" => markdown!("discrete_uniform"),
        "yssbi.distribution.erlang.sample" => markdown!("erlang"),
        "yssbi.distribution.exponential.sample" => markdown!("exponential"),
        "yssbi.distribution.fisher_snedecor.sample" => markdown!("fisher_snedecor"),
        "yssbi.distribution.gamma.sample" => markdown!("gamma"),
        "yssbi.distribution.geometric.sample" => markdown!("geometric"),
        "yssbi.distribution.hypergeometric.sample" => markdown!("hypergeometric"),
        "yssbi.distribution.inverse_gamma.sample" => markdown!("inverse_gamma"),
        "yssbi.distribution.laplace.sample" => markdown!("laplace"),
        "yssbi.distribution.log_normal.sample" => markdown!("log_normal"),
        "yssbi.distribution.negative_binomial.sample" => markdown!("negative_binomial"),
        "yssbi.distribution.normal.sample" => markdown!("normal"),
        "yssbi.distribution.pareto.sample" => markdown!("pareto"),
        "yssbi.distribution.poisson.sample" => markdown!("poisson"),
        "yssbi.distribution.students_t.sample" => markdown!("students_t"),
        "yssbi.distribution.triangular.sample" => markdown!("triangular"),
        "yssbi.distribution.uniform.sample" => markdown!("uniform"),
        "yssbi.distribution.weibull.sample" => markdown!("weibull"),

        "yssbi.plot.correlation.view" => markdown!("correlation_plot"),
        "yssbi.plot.correlogram.view" => markdown!("correlogram"),
        "yssbi.plot.ecdf.view" => markdown!("ecdf"),
        "yssbi.plot.histogram.view" => markdown!("histogram"),
        "yssbi.plot.kde.view" => markdown!("kde"),
        "yssbi.plot.line.view" => markdown!("line"),
        "yssbi.plot.scatter.view" => markdown!("scatter"),
        "yssbi.plot.boxplot.view" => markdown!("boxplot"),
        "yssbi.plot.wordcloud.view" => markdown!("wordcloud"),
        "yssbi.plot.errorbar.view" => markdown!("errorbar"),
        "yssbi.plot.pp_qq.view" => markdown!("pp_qq"),
        "yssbi.plot.roc.view" => markdown!("roc"),
        "yssbi.plot.quadrant.view" => markdown!("quadrant"),
        "yssbi.plot.pareto.view" => markdown!("pareto_plot"),
        "yssbi.plot.combination.view" => markdown!("combination"),
        "yssbi.plot.bubble.view" => markdown!("bubble"),
        "yssbi.plot.violin.view" => markdown!("violin"),
        "yssbi.plot.heatmap.view" => markdown!("heatmap"),
        "yssbi.plot.coefficient.view" => markdown!("coefficient"),

        "yssbi.statistics.test.t.one_sample" => markdown!("test_t_one_sample"),
        "yssbi.statistics.test.t.independent" => markdown!("test_t_independent"),
        "yssbi.statistics.test.t.paired" => markdown!("test_t_paired"),
        "yssbi.statistics.test.t.summary_input" => markdown!("test_t_summary"),
        "yssbi.statistics.test.z.mean" => markdown!("test_z_mean"),
        "yssbi.statistics.test.z.proportion" => markdown!("test_z_proportion"),
        "yssbi.statistics.test.binomial" => markdown!("test_binomial"),
        "yssbi.statistics.test.proportion.two" => markdown!("test_two_proportions"),
        "yssbi.statistics.test.chisquare.crosstab" => markdown!("test_chisquare_crosstab"),
        "yssbi.statistics.test.chisquare.general" => markdown!("test_chisquare_table"),
        "yssbi.statistics.test.chisquare.goodness_of_fit" => {
            markdown!("test_chisquare_goodness_of_fit")
        }
        "yssbi.statistics.test.fisher_exact" => markdown!("test_fisher_exact"),
        "yssbi.statistics.test.mcnemar" => markdown!("test_mcnemar"),
        "yssbi.statistics.test.cmh" => markdown!("test_cmh"),
        "yssbi.statistics.test.proportion.multiple" => markdown!("test_multiple_proportions"),
        "yssbi.statistics.test.poisson" => markdown!("test_poisson_rate"),
        "yssbi.statistics.test.equivalence" => markdown!("test_equivalence"),
        "yssbi.statistics.test.nonparametric.family" => markdown!("test_nonparametric_groups"),
        "yssbi.statistics.test.wilcoxon.one_sample" => markdown!("test_wilcoxon_one_sample"),
        "yssbi.statistics.test.wilcoxon.paired" => markdown!("test_wilcoxon_paired"),
        "yssbi.statistics.test.friedman" => markdown!("test_friedman"),
        "yssbi.statistics.test.runs" => markdown!("test_runs"),
        "yssbi.statistics.test.cochran_q" => markdown!("test_cochran_q"),
        "yssbi.statistics.test.mood_median" => markdown!("test_mood_median"),
        "yssbi.statistics.test.mann_kendall" => markdown!("test_mann_kendall"),
        "yssbi.statistics.test.mann_whitney" => markdown!("test_mann_whitney"),
        "yssbi.statistics.test.kruskal_wallis" => markdown!("test_kruskal_wallis"),
        "yssbi.statistics.test.levene" => markdown!("test_levene"),
        "yssbi.statistics.test.brown_forsythe" => markdown!("test_brown_forsythe"),
        "yssbi.statistics.test.bartlett" => markdown!("test_bartlett"),
        "yssbi.statistics.adf.test" => markdown!("df_adf"),
        "yssbi.statistics.test.normality" => markdown!("normality"),
        "yssbi.statistics.diagnostic.breusch_pagan" => markdown!("breusch_pagan"),
        "yssbi.statistics.diagnostic.white" => markdown!("white"),
        "yssbi.statistics.diagnostic.information_matrix" => markdown!("information_matrix"),
        "yssbi.statistics.diagnostic.reset" => markdown!("reset"),
        "yssbi.statistics.diagnostic.vif" => markdown!("vif"),
        "yssbi.statistics.diagnostic.leverage" => markdown!("leverage"),
        "yssbi.statistics.diagnostic.breusch_godfrey" => markdown!("breusch_godfrey"),
        "yssbi.statistics.diagnostic.wald" => markdown!("wald"),
        "yssbi.statistics.diagnostic.durbin_watson" => markdown!("durbin_watson"),
        "yssbi.statistics.diagnostic.ljung_box" => markdown!("ljung_box"),
        "yssbi.statistics.diagnostic.hausman" => markdown!("hausman"),
        "yssbi.statistics.timeseries.acf" => markdown!("acf"),
        "yssbi.statistics.timeseries.pacf" => markdown!("pacf"),
        "yssbi.statistics.timeseries.granger" => markdown!("granger"),
        "yssbi.statistics.timeseries.irf" => markdown!("irf"),
        "yssbi.statistics.timeseries.fevd" => markdown!("fevd"),
        "yssbi.statistics.linear.fit" => markdown!("linear_regression"),
        "yssbi.statistics.linear.summary" => markdown!("linear_regression_summary"),
        "yssbi.statistics.iv.2sls.fit" => markdown!("iv_2sls_fit"),
        "yssbi.statistics.iv.2sls.summary" => markdown!("iv_2sls_summary"),
        "yssbi.statistics.iv.liml.fit" => markdown!("iv_liml_fit"),
        "yssbi.statistics.iv.liml.summary" => markdown!("iv_liml_summary"),
        "yssbi.statistics.logit.fit" => markdown!("logit"),
        "yssbi.statistics.logit.predict" => markdown!("logit_predict"),
        "yssbi.statistics.logit.summary" => markdown!("logit_summary"),
        "yssbi.statistics.panel.fit" => markdown!("panel_fit"),
        "yssbi.statistics.panel.predict"
        | "yssbi.statistics.panel.compare"
        | "yssbi.statistics.panel.summary" => markdown!("panel_summary"),
        "yssbi.statistics.panel.did.twfe" => markdown!("panel_did"),
        "yssbi.statistics.panel.did.randomization" => markdown!("did_randomization"),
        "yssbi.statistics.prais.fit" => markdown!("prais"),
        "yssbi.statistics.prais.summary" => markdown!("prais_summary"),
        "yssbi.statistics.linear.predict" => markdown!("predict"),
        "yssbi.statistics.probit.predict" => markdown!("probit_predict"),
        "yssbi.statistics.probit.fit" => markdown!("probit"),
        "yssbi.statistics.probit.summary" => markdown!("probit_summary"),
        "yssbi.statistics.var.lag_order" => markdown!("var_varsoc"),
        "yssbi.statistics.var.fit" => markdown!("var_fit"),
        "yssbi.statistics.var.summary" => markdown!("var_summary"),
        "yssbi.statistics.vec.fit" => markdown!("vec"),
        "yssbi.statistics.vec.summary" => markdown!("vec_summary"),
        "yssbi.statistics.vec.rank_test" => markdown!("vecrank"),
        _ => return None,
    })
}

fn select_locale(documentation: Documentation, locale: &str) -> &'static str {
    let locale = locale.trim().replace('_', "-").to_ascii_lowercase();
    if locale == "zh" || locale.starts_with("zh-") {
        documentation.zh.unwrap_or(documentation.en)
    } else {
        documentation.en
    }
}
