//! Install only admitted runtime assets, publishing their catalog last.

use atomicwrites::{AllowOverwrite, AtomicFile};
use serde_json::Value;
use std::fs::{self, File};
use std::path::Path;
use yss_canonical_hash::content_sha256_reader;

pub(super) fn stage(
    repository: &Path,
    root: &Path,
    destination: &Path,
    catalog: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(destination)?;
    for dataset in catalog["datasets"]
        .as_array()
        .ok_or("missing sample entries")?
    {
        let relative = Path::new(dataset["id"].as_str().ok_or("missing sample ID")?)
            .join(format!("v{}", dataset["version"]))
            .join("data.parquet");
        let target = destination.join(&relative);
        fs::create_dir_all(target.parent().ok_or("missing destination parent")?)?;
        let mut input = File::open(root.join(relative))?;
        AtomicFile::new(&target, AllowOverwrite).write(|output| {
            std::io::copy(&mut input, output)?;
            Ok::<_, std::io::Error>(())
        })?;
        if content_sha256_reader(File::open(&target)?)?
            != dataset["sha256"].as_str().ok_or("missing sample hash")?
        {
            return Err("staged sample does not match its verified catalog".into());
        }
    }
    for (source, name) in [
        (root.join("README.md"), "README.md"),
        (
            repository.join("scripts/samples/sources.json"),
            "sources.json",
        ),
        (root.join("catalog.json"), "catalog.json"),
    ] {
        let mut input = File::open(source)?;
        AtomicFile::new(destination.join(name), AllowOverwrite).write(|output| {
            std::io::copy(&mut input, output)?;
            Ok::<_, std::io::Error>(())
        })?;
    }
    println!("staged samples: {}", destination.display());
    Ok(())
}
