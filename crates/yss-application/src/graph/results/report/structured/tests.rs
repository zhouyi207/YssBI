use super::*;
use crate::graph::results::structured::{page_with_tables, project_with_tables, table_marker};
use serde_json::{Value, json};

fn projection(value: Value) -> RuntimeValue {
    project_with_tables(&RuntimeValue::try_from(value).unwrap(), &table_marker).unwrap()
}

#[test]
fn declarations_bind_bounded_equations_and_exact_structured_paths() {
    let source = json!({
        "a/~": [{"n":1}], "equation":"y = 1 × x", "roots": [],
        "report_display":{"sections":{
            "equation":{"kind":"equation","title":"Equation","path":"/equation"},
            "table":{"kind":"table","title":"Values","path":"/a~1~0","columns":{"n":"N"}},
            "stability":{"kind":"stability","title":"Roots","path":"/roots"}
        }}
    });
    let parsed = sections(&projection(source.clone())).unwrap();
    assert_eq!(parsed.len(), 3);
    assert!(
        matches!(&parsed[0].content, ReportSectionContent::Equation(text) if text == "y = 1 × x")
    );
    let ReportSectionContent::Table(table) = &parsed[2].content else {
        panic!("table")
    };
    assert_eq!(table.part(), &ResultTablePart::Structured("/a~1~0".into()));
    assert_eq!(table.row_count, 1);
    assert!(
        sections(&projection(json!({"raw": {"value": 7}})))
            .unwrap()
            .is_empty()
    );
    for invalid in [
        json!({"kind":"table","title":"Values","path":"/a~1~0","columns":{}}),
        json!({"kind":"table","title":"Values","path":"/a~2","columns":{"n":"N"}}),
        json!({"kind":"equation","title":" ","path":"/equation"}),
        json!({"kind":"equation","title":"Equation","path":"/equation","extra":true}),
    ] {
        let mut value = source.clone();
        value["report_display"]["sections"]["table"] = invalid;
        assert!(sections(&projection(value)).is_err());
    }
    let mut oversized = source.clone();
    oversized["equation"] = json!("x".repeat(16_385));
    assert!(sections(&projection(oversized)).is_err());
    let mut mismatch = source;
    mismatch["a/~"] = json!({"kind":"tableRef","part":"structured:/roots","rowCount":0});
    assert!(sections(&projection(mismatch)).is_err());
}

#[test]
fn report_pages_preserve_all_rows_wide_integers_nulls_and_original_roots() {
    let source = RuntimeValue::try_from(json!({
        "rows": (0..205).map(|i| json!({"id":u64::MAX-i,"nullable":null})).collect::<Vec<_>>(),
        "roots":[{"re":0.5,"im":-0.75,"modulus":0.9013878188659973},{"re":1.25,"im":0,"modulus":null}],
        "invalid":[{"id":1},{"id":{"nested":true}}],
        "bad_roots":[{"re":0,"im":0},{"re":"0.5","im":0}],
        "report_display":{"sections":{
            "table":{"kind":"table","title":"Rows","path":"/rows","columns":{"id":"ID","nullable":"Nullable"}},
            "stability":{"kind":"stability","title":"Roots","path":"/roots"},
            "invalid":{"kind":"table","title":"Invalid","path":"/invalid","columns":{"id":"ID"}},
            "bad_roots":{"kind":"stability","title":"Invalid roots","path":"/bad_roots"}
        }}
    })).unwrap();
    let overview = project_with_tables(&source, &table_marker).unwrap();
    let parsed = sections(&overview).unwrap();
    let find = |title| {
        let section = parsed
            .iter()
            .find(|section| section.title == title)
            .unwrap();
        let ReportSectionContent::Table(table) = &section.content else {
            panic!("table")
        };
        table
    };
    let page = |path, offset| page_with_tables(&source, path, offset, 100, &table_marker).unwrap();
    let last = find("Rows").present(page("/rows", 200)).unwrap();
    assert_eq!(
        (
            last.table.offset,
            last.table.total_count,
            last.table.has_more
        ),
        (200, Some(205), false)
    );
    assert_eq!(last.table.values.len(), 5);
    let RuntimeValue::List(cells) = &last.table.values[0] else {
        panic!("cells")
    };
    assert!(
        matches!(&cells[0], RuntimeValue::Scalar(TabularScalar::Unsigned(value)) if *value == u64::MAX - 200)
    );
    assert!(matches!(
        &cells[1],
        RuntimeValue::Scalar(TabularScalar::Null)
    ));
    let roots = find("Roots")
        .present(page("/roots", 0))
        .unwrap()
        .roots
        .unwrap();
    assert_eq!(
        (roots[0].re, roots[0].im, roots[0].modulus),
        (0.5, -0.75, Some(0.9013878188659973))
    );
    assert_eq!((roots[1].re, roots[1].modulus), (1.25, None));
    assert!(find("Invalid").present(page("/invalid", 0)).is_err());
    assert!(
        find("Invalid roots")
            .present(page("/bad_roots", 0))
            .is_err()
    );
    assert!(find("Rows").present(page("/roots", 0)).is_err());
}
