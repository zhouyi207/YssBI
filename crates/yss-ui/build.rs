use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=locales");
    let locales = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("locales");
    let catalog = yss_i18n::codegen::Catalog::load(&locales)?;
    std::fs::write(
        PathBuf::from(env::var("OUT_DIR")?).join("messages.rs"),
        catalog.to_rust(),
    )?;
    Ok(())
}
