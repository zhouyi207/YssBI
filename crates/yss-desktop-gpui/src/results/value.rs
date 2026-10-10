//! Display conversion only; all numerical and statistical values come from Rust results.
use std::{collections::BTreeSet, sync::Arc};
use yss_application::graph::results::{ResultValueProjection, report::ResultTablePart};
use yss_data_contract::TabularScalar;
use yss_node_kernel::RuntimeValue;

pub struct ValueRow {
    pub path: String,
    pub label: String,
    pub depth: usize,
    pub value: String,
    pub expandable: bool,
    pub table: Option<ResultTablePart>,
}

pub fn display(value: &RuntimeValue) -> String {
    match value.unannotated() {
        RuntimeValue::Scalar(value) => match value {
            TabularScalar::Null => "null".into(),
            TabularScalar::Bool(value) => value.to_string(),
            TabularScalar::Integer(value) => value.to_string(),
            TabularScalar::Unsigned(value) => value.to_string(),
            TabularScalar::Float64(value) => value.as_f64().to_string(),
            TabularScalar::String(value) => serde_json::to_string(value).unwrap_or_default(),
        },
        RuntimeValue::List(values) => crate::text::format(
            "native.results.itemCount",
            &[("value0", values.len().to_string())],
        ),
        RuntimeValue::Record(values) => crate::text::format(
            "native.results.fieldCount",
            &[("value0", values.len().to_string())],
        ),
        RuntimeValue::Resource(value) => value.to_string(),
        RuntimeValue::Relation(_) => crate::text::t("native.results.dataTable").into(),
        RuntimeValue::Series(_) => crate::text::t("native.results.series").into(),
        RuntimeValue::Grouped(_) => crate::text::t("native.results.groupedData").into(),
        RuntimeValue::LinearRegression(_) => {
            crate::text::t("native.results.regressionModel").into()
        }
        RuntimeValue::Annotated(_) => unreachable!("unannotated value"),
    }
}

pub fn rows(value: &RuntimeValue, expanded: &BTreeSet<String>, tables: bool) -> Vec<ValueRow> {
    let mut rows = Vec::new();
    walk(
        value,
        "",
        crate::text::t("detail.description.result"),
        0,
        expanded,
        tables,
        &mut rows,
    );
    rows
}

fn walk(
    value: &RuntimeValue,
    path: &str,
    label: &str,
    depth: usize,
    expanded: &BTreeSet<String>,
    tables: bool,
    rows: &mut Vec<ValueRow>,
) {
    let value = value.unannotated();
    let table = tables.then(|| table_part(value)).flatten();
    let expandable =
        table.is_none() && matches!(value, RuntimeValue::Record(_) | RuntimeValue::List(_));
    rows.push(ValueRow {
        path: path.into(),
        label: label.into(),
        depth,
        value: display(value),
        expandable,
        table,
    });
    if !expandable || (depth > 0 && !expanded.contains(path)) {
        return;
    }
    match value {
        RuntimeValue::Record(record) => {
            for (key, value) in record.iter() {
                let key_path = key.replace('~', "~0").replace('/', "~1");
                walk(
                    value,
                    &format!("{path}/{key_path}"),
                    key,
                    depth + 1,
                    expanded,
                    tables,
                    rows,
                );
            }
        }
        RuntimeValue::List(values) => {
            for (index, value) in values.iter().enumerate() {
                walk(
                    value,
                    &format!("{path}/{index}"),
                    &index.to_string(),
                    depth + 1,
                    expanded,
                    tables,
                    rows,
                );
            }
        }
        _ => {}
    }
}

fn table_part(value: &RuntimeValue) -> Option<ResultTablePart> {
    let RuntimeValue::Record(record) = value else {
        return None;
    };
    if !matches!(record.get("kind")?.unannotated(), RuntimeValue::Scalar(TabularScalar::String(kind)) if kind.as_ref() == "tableRef")
    {
        return None;
    }
    let RuntimeValue::Scalar(TabularScalar::String(part)) = record.get("part")?.unannotated()
    else {
        return None;
    };
    part.parse().ok()
}

pub fn overview(projection: &ResultValueProjection) -> anyhow::Result<Arc<RuntimeValue>> {
    let json = match projection {
        ResultValueProjection::Value(value) => return Ok(Arc::new(value.clone())),
        ResultValueProjection::LinearModel(model) => serde_json::to_value(model)?,
        ResultValueProjection::LinearReport(report) => return linear_overview(report),
    };
    Ok(Arc::new(RuntimeValue::try_from(json)?))
}

pub fn linear_overview(
    report: &yss_application::graph::results::report::LinearRegressionReportProjection,
) -> anyhow::Result<Arc<RuntimeValue>> {
    let json = serde_json::json!({
        "title": report.title, "endog_name": report.endog_name, "model": report.model,
        "condition_number": report.condition_number,
        "coefficients": { "kind": "tableRef", "part": "coefficients", "rowCount": report.coefficient_count },
        "observations": { "kind": "tableRef", "part": "observations", "rowCount": report.observation_count },
    });
    Ok(Arc::new(RuntimeValue::try_from(json)?))
}
