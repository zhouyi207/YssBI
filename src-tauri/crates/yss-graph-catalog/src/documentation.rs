use yss_graph_protocol::NodeTypeId;

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

pub(crate) fn documentation(node_type_id: &NodeTypeId, locale: &str) -> Option<&'static str> {
    let documentation = mapped_documentation(node_type_id.as_str())?;
    Some(select_locale(documentation, locale))
}

fn mapped_documentation(node_type_id: &str) -> Option<Documentation> {
    Some(match node_type_id {
        "yssbi.constant.bool" => markdown!("boolean_const"),
        "yssbi.constant.int64" => markdown!("int64_const"),
        "yssbi.constant.float64" => markdown!("float64_const"),
        "yssbi.constant.string" => markdown!("string_const"),

        "yssbi.numeric.add" => markdown!("add"),
        "yssbi.numeric.subtract" => markdown!("subtract"),
        "yssbi.numeric.multiply" => markdown!("multiply"),
        "yssbi.numeric.divide" => markdown!("divide"),
        "yssbi.numeric.ln" => markdown!("ln"),
        "yssbi.numeric.log2" => markdown!("log2"),
        "yssbi.numeric.log10" => markdown!("log10"),
        "yssbi.numeric.exp" => markdown!("exp"),
        "yssbi.numeric.sqrt" => markdown!("sqrt"),
        "yssbi.numeric.square" => markdown!("square"),

        "yssbi.logic.equal" => markdown!("equal"),
        "yssbi.logic.not_equal" => markdown!("not_equal"),
        "yssbi.logic.and" => markdown!("and"),
        "yssbi.logic.or" => markdown!("or"),
        "yssbi.logic.not" => markdown!("not"),

        "yssbi.value.convert" => markdown!("convert"),
        "yssbi.data_series.convert.string_to_categorical" => {
            markdown!("string_to_categorical")
        }
        "yssbi.data_series.convert.string_to_float64" => markdown!("string_to_float64"),
        "yssbi.data_series.convert.string_to_int64" => markdown!("string_to_int64"),
        "yssbi.data_series.convert.int64_to_string" => markdown!("int64_to_string"),
        "yssbi.data_series.convert.float64_to_string" => markdown!("float64_to_string"),
        "yssbi.data_series.convert.int64_to_float64" => markdown!("int64_to_float64"),
        "yssbi.data_series.convert.float64_to_int64" => markdown!("float64_to_int64"),
        "yssbi.data_series.convert.int64_to_bool" => markdown!("int64_to_boolean"),
        "yssbi.data_series.convert.float64_to_bool" => markdown!("float64_to_boolean"),
        "yssbi.data_series.convert.categorical_to_string" => {
            markdown!("categorical_to_string")
        }
        "yssbi.data_series.convert.int64_to_categorical" => {
            markdown!("int64_to_categorical")
        }
        "yssbi.data_series.convert.categorical_to_int64" => {
            markdown!("categorical_to_int64")
        }
        "yssbi.data_series.convert.float64_to_categorical" => {
            markdown!("float64_to_categorical")
        }
        "yssbi.data_series.convert.categorical_to_float64" => {
            markdown!("categorical_to_float64")
        }

        "yssbi.project.function.call" => markdown!("call_function"),
        "yssbi.project.variable.get" => markdown!("get_variable"),
        "yssbi.debug.view" => markdown!("view"),

        "yssbi.dataframe.source.get" => markdown!("get_dataframe"),
        "yssbi.dataframe.decompose" => markdown!("decompose_dataframe"),
        "yssbi.dataframe.combine" => markdown!("combine_dataframe"),
        "yssbi.dataframe.filter" => markdown!("filter_dataframe"),
        "yssbi.dataframe.series.select" => markdown!("get_dataseries"),
        "yssbi.dataframe.series.length" => markdown!("dataseries_length"),
        "yssbi.dataframe.series.sum" => markdown!("dataseries_sum"),
        "yssbi.dataframe.series.mean" => markdown!("dataseries_mean"),
        "yssbi.dataframe.series.compare.greater" => markdown!("dataseries_gt"),
        "yssbi.dataframe.series.compare.less" => markdown!("dataseries_lt"),
        "yssbi.dataframe.series.compare.greater_equal" => markdown!("dataseries_gte"),
        "yssbi.dataframe.series.compare.less_equal" => markdown!("dataseries_lte"),
        "yssbi.dataframe.series.compare.equal" | "yssbi.dataframe.series.compare.string.equal" => {
            markdown!("dataseries_eq")
        }
        "yssbi.dataframe.series.compare.not_equal"
        | "yssbi.dataframe.series.compare.string.not_equal" => markdown!("dataseries_neq"),
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

        "yssbi.statistics.adf.test" => markdown!("df_adf"),
        "yssbi.statistics.adf.summary" => markdown!("df_adf_summary"),
        "yssbi.statistics.ols.vce.non_robust" => markdown!("vce_nonrobust"),
        "yssbi.statistics.ols.vce.hc0" => markdown!("vce_hc0"),
        "yssbi.statistics.ols.vce.hc1" => markdown!("vce_hc1"),
        "yssbi.statistics.ols.vce.hc2" => markdown!("vce_hc2"),
        "yssbi.statistics.ols.vce.hc3" => markdown!("vce_hc3"),
        "yssbi.statistics.ols.vce.fixed_scale" => markdown!("ols_fixed_scale_config"),
        "yssbi.statistics.ols.vce.cluster" => markdown!("ols_cluster_config"),
        "yssbi.statistics.ols.vce.hac" => markdown!("ols_hac_config"),
        "yssbi.statistics.ols.vce.newey_west" => markdown!("ols_newey_config"),
        "yssbi.statistics.ols.configure" => markdown!("ols_configure"),
        "yssbi.statistics.ols.fit" => markdown!("ols"),
        "yssbi.statistics.ols.summary" => markdown!("ols_summary"),
        "yssbi.statistics.gls.configure" => markdown!("gls_configure"),
        "yssbi.statistics.gls.fit" => markdown!("gls"),
        "yssbi.statistics.gls.summary" => markdown!("gls_summary"),
        "yssbi.statistics.iv.2sls.configure" => markdown!("iv_2sls_configure"),
        "yssbi.statistics.iv.2sls.summary" => markdown!("iv_2sls_summary"),
        "yssbi.statistics.iv.liml.summary" => markdown!("iv_liml_summary"),
        "yssbi.statistics.logit.configure" => markdown!("logit_configure"),
        "yssbi.statistics.logit.fit" => markdown!("logit"),
        "yssbi.statistics.logit.predict" => markdown!("logit_predict"),
        "yssbi.statistics.logit.summary" => markdown!("logit_summary"),
        "yssbi.statistics.panel.configure" => markdown!("panel_configure"),
        "yssbi.statistics.panel.vce.cluster_entity" => markdown!("panel_vce_cluster"),
        "yssbi.statistics.panel.summary" => markdown!("panel_summary"),
        "yssbi.statistics.panel.did.twfe" => markdown!("panel_did"),
        "yssbi.statistics.prais.configure" => markdown!("prais_configure"),
        "yssbi.statistics.prais.fit" => markdown!("prais"),
        "yssbi.statistics.prais.summary" => markdown!("prais_summary"),
        "yssbi.statistics.linear.predict" => markdown!("predict"),
        "yssbi.statistics.probit.predict" => markdown!("probit_predict"),
        "yssbi.statistics.probit.configure" => markdown!("probit_configure"),
        "yssbi.statistics.probit.fit" => markdown!("probit"),
        "yssbi.statistics.probit.summary" => markdown!("probit_summary"),
        "yssbi.statistics.var.lag_order" => markdown!("var_varsoc"),
        "yssbi.statistics.var.summary" => markdown!("var_summary"),
        "yssbi.statistics.vec.fit" => markdown!("vec"),
        "yssbi.statistics.vec.rank_test" => markdown!("vecrank"),
        "yssbi.statistics.wls.fit" => markdown!("wls"),
        "yssbi.statistics.wls.summary" => markdown!("wls_summary"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_language_prefix_and_falls_back_to_english() {
        let node_type_id = NodeTypeId::new("yssbi.plot.scatter.view").unwrap();

        assert_eq!(
            documentation(&node_type_id, "zh-TW").unwrap(),
            documentation(&node_type_id, "zh-CN").unwrap()
        );
        assert_eq!(
            documentation(&node_type_id, "fr-FR").unwrap(),
            documentation(&node_type_id, "en-US").unwrap()
        );
        assert_eq!(
            select_locale(
                Documentation {
                    en: "English",
                    zh: None,
                },
                "zh-CN"
            ),
            "English"
        );
    }

    #[test]
    fn leaves_unmapped_nodes_without_documentation() {
        let node_type_id = NodeTypeId::new("yssbi.unknown.node").unwrap();

        assert!(documentation(&node_type_id, "en-US").is_none());
    }
}
