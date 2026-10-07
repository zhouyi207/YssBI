//! Bounded display data derived from the authoritative statistical result.
use serde::Serialize;
use yss_sci_contract::regression::report::LinearModelSummary;

#[derive(Serialize)]
#[serde(untagged)]
pub enum DisplayValue {
    Text(String),
    Number(f64),
    Integer(usize),
}

impl From<&str> for DisplayValue {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}
impl From<f64> for DisplayValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}
impl From<usize> for DisplayValue {
    fn from(value: usize) -> Self {
        Self::Integer(value)
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DisplayFormat {
    Text,
    Number,
    Integer,
    PValue,
}

#[derive(Serialize)]
pub struct Metric {
    id: &'static str,
    label: &'static str,
    value: DisplayValue,
    format: DisplayFormat,
}

#[derive(Serialize)]
pub struct Column {
    id: &'static str,
    label: &'static str,
    format: DisplayFormat,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DisplayData {
    KeyValue {
        items: Vec<Metric>,
    },
    Table {
        columns: Vec<Column>,
        rows: Vec<Vec<DisplayValue>>,
    },
    StatCard {
        stat: Metric,
    },
}

pub fn data(
    model: &LinearModelSummary,
    condition_number: f64,
) -> std::collections::BTreeMap<&'static str, DisplayData> {
    use DisplayFormat::*;
    let metric = |id, label, value, format| Metric {
        id,
        label,
        value,
        format,
    };
    let summary = DisplayData::KeyValue {
        items: vec![
            metric("modelType", "Model", model.model_type.as_str().into(), Text),
            metric("method", "Method", model.method.as_str().into(), Text),
            metric("rSquared", "R-squared", model.r_squared.into(), Number),
            metric(
                "adjRSquared",
                "Adj. R-squared",
                model.adj_r_squared.into(),
                Number,
            ),
            metric(
                "fStatistic",
                "F-statistic",
                model.f_statistic.into(),
                Number,
            ),
            metric(
                "probFStatistic",
                "Prob (F-statistic)",
                model.prob_f_statistic.into(),
                PValue,
            ),
            metric(
                "numObservations",
                "No. Observations",
                model.num_observation.into(),
                Integer,
            ),
            metric(
                "covarianceType",
                "Covariance Type",
                model.covariance_type.as_str().into(),
                Text,
            ),
            metric("dfModel", "Df Model", model.df_model.into(), Integer),
            metric(
                "dfResidual",
                "Df Residual",
                model.df_residual.into(),
                Integer,
            ),
            metric("dfTotal", "Df Total", model.df_total.into(), Integer),
        ],
    };
    let anova = DisplayData::Table {
        columns: vec![
            Column {
                id: "source",
                label: "Source",
                format: Text,
            },
            Column {
                id: "ss",
                label: "SS",
                format: Number,
            },
            Column {
                id: "df",
                label: "df",
                format: Integer,
            },
            Column {
                id: "ms",
                label: "MS",
                format: Number,
            },
        ],
        rows: vec![
            vec![
                "Model".into(),
                model.ss_model.into(),
                model.df_model.into(),
                model.ms_model.into(),
            ],
            vec![
                "Residual".into(),
                model.ss_residual.into(),
                model.df_residual.into(),
                model.ms_residual.into(),
            ],
            vec![
                "Total".into(),
                model.ss_total.into(),
                model.df_total.into(),
                model.ms_total.into(),
            ],
        ],
    };
    std::collections::BTreeMap::from([
        ("summary", summary),
        ("anova", anova),
        (
            "conditionNumber",
            DisplayData::StatCard {
                stat: metric(
                    "conditionNumber",
                    "Condition Number",
                    condition_number.into(),
                    Number,
                ),
            },
        ),
    ])
}
