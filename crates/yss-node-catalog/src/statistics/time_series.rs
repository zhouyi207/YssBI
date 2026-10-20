//! Executable entries for the remaining time-series inventory.
use super::*;

const METHODS: &[&str] = &[
    "timeseries.arima",
    "timeseries.sarima",
    "plot.correlogram",
    "plot.time_series",
    "timeseries.ecm",
    "timeseries.arch",
    "timeseries.grey_prediction",
    "timeseries.exponential_smoothing",
    "timeseries.markov_prediction",
    "timeseries.phillips_perron",
    "timeseries.kpss",
    "timeseries.ets",
    "timeseries.holt_winters",
    "timeseries.garch",
    "timeseries.egarch",
    "timeseries.gjr_garch",
];
fn parameters(method: &str) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    let mut p = match method {
        "timeseries.arima" | "timeseries.sarima" => vec![
            nonnegative_integer_parameter("ts_p", 1)?,
            nonnegative_integer_parameter("ts_d", 1)?,
            nonnegative_integer_parameter("ts_q", 0)?,
            toggle_parameter("constant", true)?,
            decimal_parameter("ts_confidence", "0.95")?,
        ],
        "timeseries.ecm" => vec![
            nonnegative_integer_parameter("ts_lags", 0)?,
            toggle_parameter("constant", true)?,
        ],
        "timeseries.arch" | "timeseries.garch" | "timeseries.egarch" | "timeseries.gjr_garch" => {
            vec![
                positive_integer_parameter("ts_p", 1)?,
                toggle_parameter("constant", true)?,
            ]
        }
        "timeseries.exponential_smoothing" => vec![
            decimal_parameter("ts_alpha", "0.2")?,
            toggle_parameter("ts_optimize", true)?,
        ],
        "timeseries.ets" | "timeseries.holt_winters" => vec![
            decimal_parameter("ts_alpha", "0.2")?,
            decimal_parameter("ts_beta", "0.1")?,
            decimal_parameter("ts_gamma", "0.1")?,
            decimal_parameter("ts_phi", "0.98")?,
            toggle_parameter("ts_optimize", true)?,
            toggle_parameter("ts_trend", true)?,
            toggle_parameter("ts_damped", false)?,
            choice_parameter(
                "ts_seasonality",
                if method == "timeseries.ets" {
                    "none"
                } else {
                    "additive"
                },
                if method == "timeseries.ets" {
                    &["none", "additive"]
                } else {
                    &["additive", "multiplicative"]
                },
            )?,
            minimum_integer_parameter("ts_period", 12, 2)?,
        ],
        "timeseries.markov_prediction" => vec![decimal_parameter("ts_pseudocount", "0")?],
        "timeseries.phillips_perron" | "timeseries.kpss" => vec![
            nonnegative_integer_parameter("ts_bandwidth", 4)?,
            choice_parameter(
                "ts_deterministic",
                "constant",
                if method == "timeseries.kpss" {
                    &["constant", "trend"]
                } else {
                    &["none", "constant", "trend"]
                },
            )?,
        ],
        "plot.correlogram" => vec![bounded_integer_parameter("ts_maximum_lag", 20, 1, 40)?],
        _ => vec![],
    };
    if method == "timeseries.sarima" {
        p.extend([
            nonnegative_integer_parameter("ts_seasonal_p", 0)?,
            nonnegative_integer_parameter("ts_seasonal_d", 1)?,
            nonnegative_integer_parameter("ts_seasonal_q", 1)?,
            minimum_integer_parameter("ts_period", 12, 2)?,
        ]);
    }
    if matches!(
        method,
        "timeseries.garch" | "timeseries.egarch" | "timeseries.gjr_garch"
    ) {
        p.push(nonnegative_integer_parameter("ts_q", 1)?);
    }
    if method == "timeseries.egarch" {
        p.extend([
            positive_integer_parameter("ts_simulations", 1000)?,
            nonnegative_integer_parameter("ts_seed", 42)?,
        ]);
    }
    if !matches!(
        method,
        "timeseries.ecm"
            | "timeseries.phillips_perron"
            | "timeseries.kpss"
            | "plot.correlogram"
            | "plot.time_series"
    ) {
        p.push(positive_integer_parameter("ts_horizon", 10)?);
    }
    if matches!(
        method,
        "timeseries.arima"
            | "timeseries.sarima"
            | "timeseries.arch"
            | "timeseries.garch"
            | "timeseries.egarch"
            | "timeseries.gjr_garch"
            | "timeseries.exponential_smoothing"
            | "timeseries.ets"
            | "timeseries.holt_winters"
    ) {
        p.extend([
            positive_integer_parameter("max_iterations", 500)?,
            tolerance_parameter("0.000001")?,
        ]);
    }
    Ok(p)
}
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &method in METHODS {
        let id = format!("yssbi.statistics.{method}");
        let input = if method == "timeseries.markov_prediction" {
            label_series()?
        } else {
            numeric_data_series_type()
        };
        let mut ports = vec![data_input("series", "Series", input)?];
        if method == "timeseries.ecm" {
            ports.push(bounded_user_data_input(
                "x",
                "X",
                numeric_data_series_type(),
                1,
                None,
            )?);
        }
        if method == "plot.time_series" {
            ports.push(bounded_user_data_input(
                "time",
                "Time (optional)",
                numeric_data_series_type(),
                0,
                Some(1),
            )?);
        }
        ports.push(data_output(
            "result",
            "Result",
            if method.starts_with("plot.") {
                concrete("plot.data")?
            } else {
                report_type()?
            },
        )?);
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    aliases_key: Some(node_key(&id, "aliases")?),
                    category_id: sid("statistics.timeseries", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(&id, ports, vec![], vec![])?,
                parameters: assembled_parameters(&id, parameters(method)?)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: pure(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            &id,
        ));
    }
    Ok(())
}
