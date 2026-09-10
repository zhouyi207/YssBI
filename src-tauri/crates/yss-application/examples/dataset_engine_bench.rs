//! Reproducible data-engine measurements; run separately from correctness tests.
use arrow::array::{ArrayRef, Float64Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use yss_datafusion::DataFusionRuntime;
use yss_dataset_store::{DatasetCellEdit, DatasetColumnCast, DatasetStore};
use yss_relational_contract::{
    RelationComparison, RelationControl, RelationHandle, RelationLiteral, RelationPredicate,
};
use yss_sci_contract::scientific::{BackendExecutionControl, OlsRequest, ScientificBackend};

const ROWS: usize = 1_000_000;
const COLUMNS: usize = 16;
fn control() -> RelationControl {
    RelationControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(300),
        max_input_bytes: 128 * 1024 * 1024,
    }
}
fn report(name: &str, start: Instant, detail: serde_json::Value) {
    println!(
        "{}",
        serde_json::json!({"scenario":name,"elapsed_ms":start.elapsed().as_secs_f64()*1000.0,"detail":detail})
    );
}
fn scan(
    engine: &DataFusionRuntime,
    relation: &RelationHandle,
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let mut rows = 0;
    let mut maximum = 0;
    engine.visit_relation(relation, &control(), &mut |batch| {
        rows += batch.num_rows();
        maximum = maximum.max(batch.get_array_memory_size());
        Ok(())
    })?;
    Ok((rows, maximum))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os()
        .nth(1)
        .ok_or("provide an empty benchmark directory")?;
    let root = std::path::PathBuf::from(root);
    std::fs::create_dir_all(&root)?;
    let mode = std::env::args().nth(2).unwrap_or_else(|| "measure".into());
    let id = yss_database_contract::DatabaseId::from_existing(
        "00000000-0000-4000-8000-000000000001".into(),
    );
    if mode == "generate" {
        let store = DatasetStore::create(&root)?;
        let schema = Arc::new(Schema::new(
            (0..COLUMNS)
                .map(|column| Field::new(format!("c{column}"), DataType::Float64, false))
                .collect::<Vec<_>>(),
        ));
        let batches = (0..ROWS).step_by(8192).map(|offset| {
            let end = (offset + 8192).min(ROWS);
            let arrays = (0..COLUMNS)
                .map(|column| {
                    Arc::new(Float64Array::from_iter_values((offset..end).map(|row| {
                        let x = row as f64 / 1000.0;
                        match column {
                            0 => x,
                            1 => 3.0 + 2.0 * x + (x * 0.2).sin(),
                            _ => (x * (column as f64 + 0.25)).sin(),
                        }
                    }))) as ArrayRef
                })
                .collect();
            RecordBatch::try_new(schema.clone(), arrays)
        });
        let start = Instant::now();
        let committed = store.commit(store.prepare_import(
            id,
            "Benchmark",
            "generate",
            schema.clone(),
            batches,
        )?)?;
        store.acknowledge_publication(&committed.publication)?;
        report(
            "generate",
            start,
            serde_json::json!({"rows":ROWS,"columns":COLUMNS,"parquet_bytes":committed.snapshot.files().iter().map(|file|file.size_bytes).sum::<u64>()}),
        );
        return Ok(());
    }
    let start = Instant::now();
    let store = DatasetStore::open(&root)?;
    let engine = DataFusionRuntime::new(512 * 1024 * 1024, 8192)?;
    let snapshot = store.snapshot(&id)?;
    let relation = snapshot.query(&engine, "benchmark")?.relation()?;
    report(
        "open_and_plan",
        start,
        serde_json::json!({"rows":ROWS,"columns":COLUMNS,"engine_memory_bytes":512*1024*1024}),
    );
    for name in ["first_page", "repeat_page"] {
        let start = Instant::now();
        let page = relation.page(0, 1000, &control())?;
        report(
            name,
            start,
            serde_json::json!({"rows":page.row_count,"has_more":page.has_more}),
        );
    }
    let narrow = relation.project(&["c0".into(), "c1".into()])?;
    for (name, relation) in [("narrow_scan", &narrow), ("full_scan", &relation)] {
        let start = Instant::now();
        let (rows, maximum) = scan(&engine, relation)?;
        report(
            name,
            start,
            serde_json::json!({"rows":rows,"largest_batch_buffer_bytes":maximum}),
        );
    }
    let filtered = narrow.filter(&RelationPredicate {
        column: "c0".into(),
        comparison: RelationComparison::Greater,
        value: Some(RelationLiteral::Decimal("999".into())),
    })?;
    let start = Instant::now();
    let (rows, maximum) = scan(&engine, &filtered)?;
    report(
        "selective_filter",
        start,
        serde_json::json!({"rows":rows,"largest_batch_buffer_bytes":maximum}),
    );
    let start = Instant::now();
    let edited = store.commit(store.prepare_cell_edit(
        &snapshot,
        &engine,
        "edit",
        DatasetCellEdit {
            row_id: 0,
            column: "c0",
            value: serde_json::json!(1001),
        },
        &control(),
    )?)?;
    store.acknowledge_publication(&edited.publication)?;
    report(
        "sparse_edit_commit",
        start,
        serde_json::json!({"edited_cells":1}),
    );
    let start = Instant::now();
    let filtered = edited
        .snapshot
        .query(&engine, "benchmark")?
        .relation()?
        .filter(&RelationPredicate {
            column: "c0".into(),
            comparison: RelationComparison::Greater,
            value: Some(RelationLiteral::Decimal("999".into())),
        })?;
    let (rows, maximum) = scan(&engine, &filtered)?;
    report(
        "edited_filter",
        start,
        serde_json::json!({"rows":rows,"largest_batch_buffer_bytes":maximum}),
    );
    let sample = narrow.limit(0, 100_000)?;
    let start = Instant::now();
    let mut columns = sample.numeric_columns(
        &[sample.select_series("c1")?, sample.select_series("c0")?],
        &control(),
    )?;
    report(
        "ols_input",
        start,
        serde_json::json!({"rows":columns[0].len(),"columns":2}),
    );
    let response = columns.remove(0);
    let start = Instant::now();
    let result = yss_sci_runtime::SciRuntimeBackend::new().ols(
        OlsRequest {
            response,
            predictors: columns,
            options: Default::default(),
        },
        &BackendExecutionControl::from_shared(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(300),
        ),
    )?;
    report(
        "ols_matrix",
        start,
        serde_json::json!({"rows":result.fitted.len(),"coefficients":result.coefficients}),
    );
    let start = Instant::now();
    let compact = store.commit(store.prepare_compaction(
        &edited.snapshot,
        &engine,
        "compact",
        &control(),
    )?)?;
    store.acknowledge_publication(&compact.publication)?;
    report(
        "compaction",
        start,
        serde_json::json!({"rows":compact.snapshot.metadata().row_count}),
    );
    let start = Instant::now();
    let cast = store.commit(store.prepare_cast_column(
        &compact.snapshot,
        &engine,
        "cast",
        DatasetColumnCast {
            column: "c0",
            data_type: DataType::Float32,
            force: false,
        },
        &control(),
    )?)?;
    store.acknowledge_publication(&cast.publication)?;
    report(
        "column_cast",
        start,
        serde_json::json!({"rows":cast.snapshot.metadata().row_count}),
    );
    Ok(())
}
