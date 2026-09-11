//! Offline preparation of the application's bundled sample resources.

use std::fs::{self, File};
use std::path::Path;

use arrow::record_batch::RecordBatchReader;
use serde::Deserialize;
use serde_json::{Value, json};
use yss_canonical_hash::content_sha256_reader;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Source {
    id: String,
    name: String,
    version: u32,
    source_file: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let check = match args.as_slice() {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => return Err("expected no arguments or --check".into()),
    };
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let root = repository.join("src-tauri/resources/samples");
    let catalog_path = root.join("catalog.json");
    let previous_catalog: Option<Value> = if catalog_path.exists() {
        Some(serde_json::from_reader(File::open(&catalog_path)?)?)
    } else {
        None
    };
    let definitions: Vec<Source> =
        serde_json::from_reader(File::open(repository.join("scripts/samples/sources.json"))?)?;
    let mut ids = std::collections::HashSet::new();
    let mut datasets = Vec::new();
    for source in definitions {
        if source.id.is_empty()
            || !source
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || !ids.insert(source.id.clone())
            || source.version == 0
            || source.name.trim().is_empty()
            || Path::new(&source.source_file)
                .file_name()
                .and_then(|name| name.to_str())
                != Some(source.source_file.as_str())
        {
            return Err("invalid sample definition".into());
        }
        let input = root.join(&source.source_file);
        let output = root
            .join(&source.id)
            .join(format!("v{}", source.version))
            .join("data.parquet");
        let source_hash = content_sha256_reader(File::open(&input)?)?;
        if !check {
            if let Some(previous) = previous_catalog
                .as_ref()
                .and_then(|catalog| catalog["datasets"].as_array())
                .and_then(|entries| {
                    entries.iter().find(|entry| {
                        entry["id"] == source.id && entry["version"] == source.version
                    })
                })
                && previous["sourceSha256"] != source_hash
            {
                return Err(format!(
                    "{} changed: increment its sample version before rebuilding",
                    source.id
                )
                .into());
            }
            // Inspect all input rows once during preparation, so late nulls and fractional
            // values cannot change the schema when users import the released artifact.
            let reader = yss_tabular_io::read_csv_batches(&input, b',', true, usize::MAX, 10_000)?;
            fs::create_dir_all(output.parent().ok_or("missing output parent")?)?;
            let mut writer =
                yss_tabular_io::ParquetBatchWriter::new(File::create(&output)?, reader.schema())?;
            for batch in reader {
                writer.write(&batch?)?;
            }
            writer.finish()?;
        }
        let reader = yss_tabular_io::read_parquet_batches(&output, 10_000, None)?;
        let schema = reader.schema();
        if yss_tabular_arrow::timezone_free_schema(&schema) != *schema {
            return Err(format!(
                "{} contains timezone-bearing timestamps; rebuild its sample resources",
                source.id
            )
            .into());
        }
        let mut rows = 0usize;
        for batch in reader {
            rows += batch?.num_rows();
        }
        let bytes = fs::metadata(&output)?.len();
        let digest = content_sha256_reader(File::open(&output)?)?;
        println!(
            "{}: {} rows, {} columns, {} bytes",
            source.id,
            rows,
            schema.fields().len(),
            bytes
        );
        datasets.push(json!({
            "id": source.id,
            "name": source.name,
            "version": source.version,
            "sourceFile": source.source_file,
            "sourceSha256": source_hash,
            "rowCount": rows,
            "columnCount": schema.fields().len(),
            "byteSize": bytes,
            "sha256": digest,
        }));
    }
    let catalog = json!({ "formatVersion": 1, "datasets": datasets });
    if check {
        let existing = previous_catalog.ok_or("sample catalog missing: run pnpm samples:build")?;
        if existing != catalog {
            return Err(
                "sample catalog drift: run pnpm samples:build and review the generated resources"
                    .into(),
            );
        }
    } else {
        fs::write(
            catalog_path,
            format!("{}\n", serde_json::to_string_pretty(&catalog)?),
        )?;
    }
    Ok(())
}
