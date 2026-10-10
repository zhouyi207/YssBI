use std::{env, fs::File, io::Write, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=assets");
    let assets = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("assets");
    let catalogs = rust_i18n_support::try_load_locales(
        assets.to_str().ok_or("locale asset path is not UTF-8")?,
        |_| false,
        true,
    )?;
    let destination = PathBuf::from(env::var("OUT_DIR")?).join("desktop_locales.rs");
    let mut output = File::create(destination)?;
    writeln!(
        output,
        "{{ let mut backend = yss_i18n::SimpleBackend::new();"
    )?;
    for (locale, messages) in catalogs {
        writeln!(
            output,
            "backend.add_translations(std::borrow::Cow::Borrowed({locale:?}), std::collections::HashMap::from(["
        )?;
        for (key, value) in messages {
            writeln!(
                output,
                "(std::borrow::Cow::Borrowed({key:?}), std::borrow::Cow::Borrowed({value:?})),"
            )?;
        }
        writeln!(output, "]));")?;
    }
    writeln!(output, "backend }}")?;
    Ok(())
}
