//! Compiles the node catalog's locale files and authored node help into static tables.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MARKER: &str = "<!-- nodes:";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let out = PathBuf::from(std::env::var("OUT_DIR")?);
    let locales = manifest.join("locales");
    let documentation = manifest.join("../../docs/src/nodes");
    println!("cargo:rerun-if-changed={}", locales.display());
    println!("cargo:rerun-if-changed={}", documentation.display());
    let catalog = yss_i18n::codegen::Catalog::load(&locales)?;
    std::fs::write(out.join("messages.rs"), catalog.to_rust())?;
    std::fs::write(
        out.join("documentation.rs"),
        documentation_table(&documentation)?,
    )?;
    Ok(())
}

/// Pages of the fallback locale's language declare the node types they document; every
/// other language directory translates exactly those pages under the same file names.
fn documentation_table(root: &Path) -> Result<String, String> {
    let source = yss_i18n::language(yss_i18n::FALLBACK_LOCALE);
    let mut languages = BTreeMap::new();
    for entry in read_dir(root)? {
        if entry.is_dir() {
            let language = file_name(&entry)?;
            if !yss_i18n::LOCALES
                .iter()
                .any(|locale| yss_i18n::language(locale) == language)
            {
                return Err(format!(
                    "{}: `{language}` is not the language of a supported locale",
                    entry.display()
                ));
            }
            let mut pages = BTreeMap::new();
            for page in read_dir(&entry)? {
                if page.extension().and_then(|extension| extension.to_str()) == Some("md") {
                    let text = std::fs::read_to_string(&page)
                        .map_err(|error| format!("{}: {error}", page.display()))?;
                    pages.insert(file_name(&page)?, (page, text));
                }
            }
            languages.insert(language, pages);
        }
    }
    let default = languages
        .get(source)
        .ok_or_else(|| format!("{}: missing `{source}` pages", root.display()))?;
    for (language, pages) in &languages {
        if pages.keys().ne(default.keys()) {
            return Err(format!(
                "{}/{language} must translate exactly the `{source}` pages",
                root.display()
            ));
        }
    }
    let mut nodes = BTreeMap::<String, &str>::new();
    for (name, (path, text)) in default {
        let marker = text
            .lines()
            .next()
            .and_then(|line| line.strip_prefix(MARKER))
            .and_then(|line| line.strip_suffix("-->"))
            .ok_or_else(|| {
                format!(
                    "{}: the first line must be `{MARKER} <ids> -->`",
                    path.display()
                )
            })?;
        for node in marker.split_whitespace() {
            if nodes.insert(node.to_owned(), name).is_some() {
                return Err(format!("{}: {node} is documented twice", path.display()));
            }
        }
    }
    let mut out = String::from("&[");
    for (node, name) in &nodes {
        out.push_str(&format!("({node:?}, &["));
        for (language, pages) in &languages {
            let Some((_, text)) = pages.get(*name) else {
                return Err(format!(
                    "{}/{language}/{name}.md is missing",
                    root.display()
                ));
            };
            let body = text
                .strip_prefix(MARKER)
                .and_then(|text| text.split_once('\n'))
                .map_or(text.as_str(), |(_, body)| body);
            out.push_str(&format!("({language:?}, {body:?}),"));
        }
        out.push_str("]),");
    }
    out.push(']');
    Ok(out)
}

fn read_dir(path: &Path) -> Result<Vec<PathBuf>, String> {
    std::fs::read_dir(path)
        .and_then(|entries| {
            entries
                .map(|entry| entry.map(|entry| entry.path()))
                .collect()
        })
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn file_name(path: &Path) -> Result<String, String> {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("{}: file name is not UTF-8", path.display()))
}
