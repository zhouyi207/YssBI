//! Compiles the diagnostic message templates in `locales/` into a static table.
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let locales = manifest.join("locales");
    println!("cargo:rerun-if-changed={}", locales.display());
    let catalog = yss_i18n::codegen::Catalog::load(&locales)?;
    std::fs::write(
        PathBuf::from(std::env::var("OUT_DIR")?).join("messages.rs"),
        catalog.to_rust(),
    )?;
    Ok(())
}
