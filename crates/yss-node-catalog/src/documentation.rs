use yss_node_protocol::NodeTypeId;

#[derive(Clone, Copy)]
struct Documentation {
    en: &'static str,
    zh: &'static str,
}

macro_rules! markdown {
    ($slug:literal) => {
        Documentation {
            en: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../docs/src/nodes/en/",
                $slug,
                ".md"
            )),
            zh: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../docs/src/nodes/zh/",
                $slug,
                ".md"
            )),
        }
    };
}

/// Localized help for one built-in definition, independent of project resources.
pub fn node_documentation(node_type_id: &NodeTypeId, locale: &str) -> Option<Box<str>> {
    mapped_documentation(node_type_id.as_str())
        .map(|documentation| select_locale(documentation, locale).into())
        .or_else(|| super::statistics::inventory_documentation(node_type_id.as_str(), locale))
        .or_else(|| super::dataframe::inventory_documentation(node_type_id.as_str(), locale))
        .or_else(|| super::dataframe::aggregation_documentation(node_type_id.as_str(), locale))
        .or_else(|| super::dataframe::transformation_documentation(node_type_id.as_str(), locale))
}

fn mapped_documentation(node_type_id: &str) -> Option<Documentation> {
    Some(match node_type_id {
        "yssbi.statistics.describe" => markdown!("describe"),
        "yssbi.dataframe.labels" => markdown!("data_labels"),
        "yssbi.statistics.workflow.mediation" => markdown!("path_mediation"),
        "yssbi.statistics.workflow.moderated_mediation" => markdown!("path_moderated_mediation"),
        "yssbi.statistics.sem.path" => markdown!("path_recursive"),
        "yssbi.statistics.power.principles" => markdown!("power_principles"),
        "yssbi.statistics.power.mean_difference" => markdown!("power_mean_difference"),
        "yssbi.statistics.power.paired" => markdown!("power_paired"),
        "yssbi.statistics.power.variance" => markdown!("power_variance"),
        "yssbi.statistics.power.proportion" => markdown!("power_proportion"),
        "yssbi.statistics.power.proportion_difference" => markdown!("power_proportion_difference"),
        "yssbi.statistics.power.correlation" => markdown!("power_correlation"),
        "yssbi.statistics.power.anova" => markdown!("power_anova"),
        "yssbi.statistics.power.linear_regression" => markdown!("power_linear_regression"),
        "yssbi.statistics.power.generalized_model" => markdown!("power_generalized_model"),
        "yssbi.statistics.power.logistic" => markdown!("power_logistic"),
        "yssbi.statistics.power.cox" => markdown!("power_cox"),
        "yssbi.statistics.power.logrank" => markdown!("power_logrank"),
        "yssbi.statistics.power.cluster_randomized" => markdown!("power_cluster_randomized"),
        "yssbi.statistics.power.noninferiority" => markdown!("power_noninferiority"),
        "yssbi.statistics.power.equivalence" => markdown!("power_equivalence"),
        "yssbi.statistics.survey.weights" => markdown!("survey_weights"),
        "yssbi.statistics.survey.mean_proportion" => markdown!("survey_mean_proportion"),
        "yssbi.statistics.survey.stratified" => markdown!("survey_stratified"),
        "yssbi.statistics.survey.clustered" => markdown!("survey_clustered"),
        "yssbi.statistics.survey.linear_regression" => markdown!("survey_linear_regression"),
        "yssbi.statistics.survey.logistic" => markdown!("survey_logistic"),
        "yssbi.statistics.survey.poisson" => markdown!("survey_poisson"),
        "yssbi.statistics.workflow.moderation" => markdown!("path_moderation"),
        "yssbi.statistics.workflow.moderation_advanced" => markdown!("path_moderation_advanced"),
        "yssbi.statistics.doe.range_analysis" => markdown!("doe_range_analysis"),
        "yssbi.statistics.doe.family" => markdown!("doe_full_factorial"),
        "yssbi.statistics.doe.orthogonal" => markdown!("doe_orthogonal"),
        "yssbi.statistics.doe.uniform_design" => markdown!("doe_uniform_design"),
        "yssbi.statistics.doe.response_surface" => markdown!("doe_response_surface"),
        "yssbi.statistics.doe.dose_response" => markdown!("doe_dose_response"),
        "yssbi.statistics.plot.control_chart" => markdown!("quality_control_chart"),
        "yssbi.statistics.quality.process_capability" => markdown!("quality_process_capability"),
        "yssbi.statistics.quality.measurement_system" => markdown!("quality_measurement_system"),
        "yssbi.statistics.psychometrics.reliability" => markdown!("psychometrics_reliability"),
        "yssbi.statistics.psychometrics.validity" => markdown!("psychometrics_validity"),
        "yssbi.statistics.psychometrics.content_validity" => {
            markdown!("psychometrics_content_validity")
        }
        "yssbi.statistics.psychometrics.item_analysis" => markdown!("psychometrics_item_analysis"),
        "yssbi.statistics.decision.conjoint" => markdown!("decision_conjoint"),
        "yssbi.statistics.workflow.delphi" => markdown!("decision_delphi"),
        "yssbi.statistics.decision.ahp" => markdown!("decision_ahp"),
        "yssbi.statistics.decision.fahp" => markdown!("decision_fahp"),
        "yssbi.statistics.decision.dematel" => markdown!("decision_dematel"),
        "yssbi.statistics.decision.ism" => markdown!("decision_ism"),
        "yssbi.statistics.decision.fuzzy_evaluation" => markdown!("decision_fuzzy_evaluation"),
        "yssbi.statistics.decision.turf" => markdown!("decision_turf"),
        "yssbi.statistics.decision.psm" => markdown!("decision_psm"),
        "yssbi.statistics.decision.nps" => markdown!("decision_nps"),
        "yssbi.statistics.decision.kano" => markdown!("decision_kano"),
        "yssbi.statistics.decision.rfm" => markdown!("decision_rfm"),
        "yssbi.statistics.decision.vikor" => markdown!("decision_vikor"),
        "yssbi.statistics.decision.coupling_coordination" => {
            markdown!("decision_coupling_coordination")
        }
        "yssbi.statistics.decision.obstacle_degree" => markdown!("decision_obstacle_degree"),
        "yssbi.statistics.decision.weights" => markdown!("decision_weights"),
        "yssbi.statistics.decision.entropy_weight" => markdown!("decision_entropy_weight"),
        "yssbi.statistics.decision.critic" => markdown!("decision_critic"),
        "yssbi.statistics.decision.information_weight" => markdown!("decision_information_weight"),
        "yssbi.statistics.decision.independence_weight" => {
            markdown!("decision_independence_weight")
        }
        "yssbi.statistics.decision.composite_index" => markdown!("decision_composite_index"),
        "yssbi.statistics.decision.topsis" => markdown!("decision_topsis"),
        "yssbi.statistics.decision.grey_relational" => markdown!("decision_grey_relational"),
        "yssbi.statistics.decision.wrsr" => markdown!("decision_wrsr"),
        "yssbi.statistics.decision.efficacy_coefficient" => {
            markdown!("decision_efficacy_coefficient")
        }
        "yssbi.statistics.workflow.entropy_topsis" => markdown!("decision_entropy_topsis"),
        "yssbi.statistics.inference.confidence_interval" => {
            markdown!("inference_confidence_interval")
        }
        "yssbi.statistics.inference.cluster_robust" => markdown!("inference_cluster_robust"),
        "yssbi.statistics.postestimation.adjusted_predictions" => {
            markdown!("inference_adjusted_predictions")
        }
        "yssbi.statistics.posthoc.multiple_comparisons" => {
            markdown!("inference_multiple_comparisons")
        }
        "yssbi.statistics.meta.continuous" => markdown!("meta_continuous"),
        "yssbi.statistics.meta.binary" => markdown!("meta_binary"),
        "yssbi.statistics.meta.single_proportion" => markdown!("meta_single_proportion"),
        "yssbi.statistics.meta.mean" => markdown!("meta_mean"),
        "yssbi.statistics.meta.correlation" => markdown!("meta_correlation"),
        "yssbi.statistics.meta.or_hr" => markdown!("meta_or_hr"),
        "yssbi.statistics.meta.combine_p" => markdown!("meta_combine_p"),
        "yssbi.statistics.meta.inverse_variance" => markdown!("meta_inverse_variance"),
        "yssbi.statistics.meta.fixed_effect" => markdown!("meta_fixed_effect"),
        "yssbi.statistics.meta.random_effect" => markdown!("meta_random_effect"),
        "yssbi.statistics.meta.cochran_q" => markdown!("meta_cochran_q"),
        "yssbi.statistics.meta.i_squared" => markdown!("meta_i_squared"),
        "yssbi.statistics.meta.tau_squared" => markdown!("meta_tau_squared"),
        "yssbi.statistics.meta.regression" => markdown!("meta_regression"),
        "yssbi.statistics.meta.egger" => markdown!("meta_egger"),
        "yssbi.statistics.meta.begg" => markdown!("meta_begg"),
        "yssbi.statistics.meta.leave_one_out" => markdown!("meta_leave_one_out"),
        "yssbi.statistics.meta.sensitivity" => markdown!("meta_sensitivity"),
        "yssbi.statistics.plot.forest" => markdown!("meta_forest"),
        "yssbi.statistics.plot.funnel" => markdown!("meta_funnel"),
        "yssbi.statistics.diagnostic.collinearity" => markdown!("diagnostic_collinearity"),
        "yssbi.statistics.diagnostic.nri_idi" => markdown!("diagnostic_nri_idi"),
        "yssbi.statistics.diagnostic.harman" => markdown!("diagnostic_harman"),
        "yssbi.statistics.diagnostic.residual" => markdown!("diagnostic_residual"),
        "yssbi.statistics.diagnostic.cooks_distance" => markdown!("diagnostic_cooks_distance"),
        "yssbi.statistics.diagnostic.aic" => markdown!("diagnostic_aic"),
        "yssbi.statistics.diagnostic.bic" => markdown!("diagnostic_bic"),
        "yssbi.statistics.diagnostic.lr" => markdown!("diagnostic_lr"),
        "yssbi.statistics.diagnostic.score_lm" => markdown!("diagnostic_score_lm"),
        "yssbi.statistics.diagnostic.nested_comparison" => {
            markdown!("diagnostic_nested_comparison")
        }
        "yssbi.statistics.diagnostic.ph" => markdown!("diagnostic_ph"),
        "yssbi.statistics.timeseries.arima" => markdown!("timeseries_arima"),
        "yssbi.statistics.timeseries.sarima" => markdown!("timeseries_sarima"),
        "yssbi.statistics.timeseries.ecm" => markdown!("timeseries_ecm"),
        "yssbi.statistics.timeseries.arch" => markdown!("timeseries_arch"),
        "yssbi.statistics.timeseries.garch" => markdown!("timeseries_garch"),
        "yssbi.statistics.timeseries.egarch" => markdown!("timeseries_egarch"),
        "yssbi.statistics.timeseries.gjr_garch" => markdown!("timeseries_gjr_garch"),
        "yssbi.statistics.timeseries.grey_prediction" => markdown!("timeseries_grey_prediction"),
        "yssbi.statistics.timeseries.exponential_smoothing" => {
            markdown!("timeseries_exponential_smoothing")
        }
        "yssbi.statistics.timeseries.ets" => markdown!("timeseries_ets"),
        "yssbi.statistics.timeseries.holt_winters" => markdown!("timeseries_holt_winters"),
        "yssbi.statistics.timeseries.markov_prediction" => {
            markdown!("timeseries_markov_prediction")
        }
        "yssbi.statistics.timeseries.phillips_perron" => markdown!("timeseries_phillips_perron"),
        "yssbi.statistics.timeseries.kpss" => markdown!("timeseries_kpss"),
        "yssbi.statistics.plot.time_series" => markdown!("timeseries_plot"),
        "yssbi.statistics.plot.correlogram" => markdown!("timeseries_correlogram"),
        "yssbi.statistics.spatial.weights" => markdown!("spatial_weights"),
        "yssbi.statistics.spatial.moran" => markdown!("spatial_moran"),
        "yssbi.statistics.spatial.ols" => markdown!("spatial_ols"),
        "yssbi.statistics.spatial.slm" => markdown!("spatial_slm"),
        "yssbi.statistics.spatial.sem" => markdown!("spatial_sem"),
        "yssbi.statistics.spatial.sac" => markdown!("spatial_sac"),
        "yssbi.statistics.spatial.sdm" => markdown!("spatial_sdm"),
        "yssbi.statistics.spatial.sdem" => markdown!("spatial_sdem"),
        "yssbi.statistics.spatial.slx" => markdown!("spatial_slx"),
        "yssbi.statistics.spatial.panel" => markdown!("spatial_panel"),
        "yssbi.statistics.survival.kaplan_meier" => markdown!("survival_kaplan_meier"),
        "yssbi.statistics.survival.nelson_aalen" => markdown!("survival_nelson_aalen"),
        "yssbi.statistics.survival.logrank" => markdown!("survival_logrank"),
        "yssbi.statistics.survival.cox" => markdown!("survival_cox"),
        "yssbi.statistics.survival.exponential" => markdown!("survival_exponential"),
        "yssbi.statistics.survival.weibull" => markdown!("survival_weibull"),
        "yssbi.statistics.survival.lognormal" => markdown!("survival_lognormal"),
        "yssbi.statistics.survival.loglogistic" => markdown!("survival_loglogistic"),
        "yssbi.statistics.survival.aft" => markdown!("survival_aft"),
        "yssbi.statistics.survival.competing_risks" => markdown!("survival_competing_risks"),
        "yssbi.statistics.survival.time_dependent_cox" => markdown!("survival_time_dependent_cox"),
        "yssbi.statistics.workflow.subgroup" => markdown!("survival_subgroup"),
        "yssbi.statistics.plot.calibration" => markdown!("survival_calibration"),
        "yssbi.statistics.plot.decision_curve" => markdown!("survival_decision_curve"),
        "yssbi.statistics.plot.nomogram" => markdown!("survival_nomogram"),
        "yssbi.statistics.econometrics.gmm" => markdown!("causal_gmm"),
        "yssbi.statistics.causal.rdd" => markdown!("causal_rdd"),
        "yssbi.statistics.causal.psm" => markdown!("causal_psm"),
        "yssbi.statistics.econometrics.heckman_two_step" => markdown!("causal_heckman"),
        "yssbi.statistics.test.heterogeneity" => markdown!("causal_heterogeneity"),
        "yssbi.statistics.econometrics.sfa" => markdown!("causal_sfa"),
        "yssbi.statistics.econometrics.sur" => markdown!("causal_sur"),
        "yssbi.statistics.causal.ipw" => markdown!("causal_ipw"),
        "yssbi.statistics.causal.regression_adjustment" => {
            markdown!("causal_regression_adjustment")
        }
        "yssbi.statistics.causal.aipw" => markdown!("causal_aipw"),
        "yssbi.statistics.causal.ate" => markdown!("causal_ate"),
        "yssbi.statistics.causal.att" => markdown!("causal_att"),
        "yssbi.statistics.causal.synthetic_control" => markdown!("causal_synthetic_control"),
        "yssbi.statistics.econometrics.panel.fe" => markdown!("panel_fe"),
        "yssbi.statistics.econometrics.panel.re" => markdown!("panel_re"),
        "yssbi.statistics.econometrics.panel.fd" => markdown!("panel_fd"),
        "yssbi.statistics.econometrics.panel.between" => markdown!("panel_between"),
        "yssbi.statistics.econometrics.panel.dynamic" => markdown!("panel_dynamic"),
        "yssbi.statistics.econometrics.panel.unit_root" => markdown!("panel_unit_root"),
        "yssbi.statistics.econometrics.panel.cointegration" => markdown!("panel_cointegration"),
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
        "yssbi.statistics.longitudinal.gee" => markdown!("longitudinal_gee"),
        "yssbi.statistics.mixed.hlm" => markdown!("mixed_hlm"),
        "yssbi.statistics.mixed.lmm" => markdown!("mixed_lmm"),
        "yssbi.statistics.mixed.random_intercept" => markdown!("mixed_random_intercept"),
        "yssbi.statistics.mixed.random_slope" => markdown!("mixed_random_slope"),
        "yssbi.statistics.mixed.crossed_effects" => markdown!("mixed_crossed_effects"),
        "yssbi.statistics.mixed.glmm" => markdown!("mixed_glmm"),
        "yssbi.statistics.mixed.logistic" => markdown!("mixed_logistic"),
        "yssbi.statistics.mixed.poisson" => markdown!("mixed_poisson"),
        "yssbi.statistics.mixed.negative_binomial" => markdown!("mixed_negative_binomial"),
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

        "yssbi.value.to_numeric" => markdown!("to_numeric"),
        "yssbi.value.to_text" => markdown!("to_text"),
        "yssbi.value.to_categorical" => markdown!("to_categorical"),
        "yssbi.value.to_ordinal" => markdown!("to_ordinal"),
        "yssbi.value.to_binary" => markdown!("to_binary"),
        "yssbi.value.to_datetime" => markdown!("to_datetime"),
        "yssbi.value.to_identifier" => markdown!("to_identifier"),
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
    if is_chinese_locale(locale) {
        documentation.zh
    } else {
        documentation.en
    }
}

pub(crate) fn is_chinese_locale(locale: &str) -> bool {
    locale
        .trim()
        .split(['-', '_'])
        .next()
        .is_some_and(|language| language.eq_ignore_ascii_case("zh"))
}
