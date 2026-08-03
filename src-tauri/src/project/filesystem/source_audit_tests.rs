use std::path::{Path, PathBuf};

fn rust_sources(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn is_test_only_source(relative: &str) -> bool {
    relative.rsplit('/').next() == Some("tests.rs")
        || relative.ends_with("_tests.rs")
        || relative.ends_with("production_tests.rs")
}

fn production_source(source: &str) -> String {
    let marker = "#[cfg(test)]";
    let mut remaining = source;
    let mut production = String::new();
    while let Some(offset) = remaining.find(marker) {
        production.push_str(&remaining[..offset]);
        let attributed = &remaining[offset + marker.len()..];
        let item_start = attributed.len() - attributed.trim_start().len();
        let item = &attributed[item_start..];
        let Some(item_end) = rust_item_end(item) else {
            break;
        };
        remaining = &item[item_end..];
    }
    production.push_str(remaining);
    production
}

fn rust_item_end(item: &str) -> Option<usize> {
    let bytes = item.as_bytes();
    let opening = item.find('{');
    let semicolon = item.find(';');
    if semicolon.is_some_and(|end| opening.is_none_or(|brace| end < brace)) {
        return semicolon.map(|end| end + 1);
    }
    let opening = opening?;
    let mut depth = 0_u32;
    let mut index = opening;
    let mut string = false;
    let mut character = false;
    let mut line_comment = false;
    let mut block_comment = 0_u32;
    while index < bytes.len() {
        let byte = bytes[index];
        let next = bytes.get(index + 1).copied();
        if line_comment {
            line_comment = byte != b'\n';
        } else if block_comment > 0 {
            if byte == b'/' && next == Some(b'*') {
                block_comment += 1;
                index += 1;
            } else if byte == b'*' && next == Some(b'/') {
                block_comment -= 1;
                index += 1;
            }
        } else if string {
            if byte == b'\\' {
                index += 1;
            } else if byte == b'"' {
                string = false;
            }
        } else if character {
            if byte == b'\\' {
                index += 1;
            } else if byte == b'\'' {
                character = false;
            }
        } else if byte == b'/' && next == Some(b'/') {
            line_comment = true;
            index += 1;
        } else if byte == b'/' && next == Some(b'*') {
            block_comment = 1;
            index += 1;
        } else {
            match byte {
                b'"' => string = true,
                b'\'' if looks_like_char_literal(bytes, index) => character = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index + 1);
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    None
}

fn looks_like_char_literal(bytes: &[u8], index: usize) -> bool {
    match bytes.get(index + 1).copied() {
        Some(b'\\') => bytes.get(index + 3) == Some(&b'\''),
        Some(_) => bytes.get(index + 2) == Some(&b'\''),
        None => false,
    }
}

#[test]
fn production_project_document_io_is_owned_by_filesystem_modules() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_sources(&source_root, &mut files);

    let removed_symbols = [
        concat!("with_project_filesystem_", "transaction"),
        concat!("with_current_project_filesystem_", "transaction"),
        concat!("filesystem_", "transactions"),
        concat!("ProjectFilesystem", "Snapshot"),
        concat!("GraphRenameDisk", "Rollback"),
        concat!("save_project_", "to_file"),
        concat!("save_project_graph_", "to_file"),
        concat!("save_project_as_", "to_directory"),
        concat!("duplicate_project_graph_", "file"),
        concat!("remove_project_graph_from_", "file"),
        concat!("cascade_graph_path_references_on_", "disk"),
        concat!("delete_project_", "directory"),
    ];
    let removed_architecture = [
        concat!("get_editor_schema_", "command"),
        concat!("useNodeRegistry", "Store"),
        concat!("useGlobalTypeSystem", "Store"),
    ];
    let read_only_scanners = [
        "project/project_io.rs",
        "project/project_watcher.rs",
        "project/project_scan.rs",
        "project/graph_resource_index.rs",
        "project/worksheet_io.rs",
    ];
    let duckdb_files = [
        "application/database.rs",
        "database/duckdb_analytics.rs",
        "database/duckdb_reader.rs",
        "database/excel_reader.rs",
        "project/project_state_database.rs",
        "tabular/dataframe_io.rs",
    ];
    let non_project_files = [
        "application/bayes.rs",
        "julia/worker.rs",
        "log/log_manager.rs",
        "project/project_registry.rs",
        "sci/backends/julia/bayes/fit.rs",
        "sci/backends/julia/time_series/acf_pacf.rs",
        "window_state/mod.rs",
    ];
    let write_patterns = [
        "std::fs::write",
        "std::fs::rename",
        "std::fs::copy",
        "std::fs::remove_",
        "std::fs::create_",
        "fs::write",
        "fs::rename",
        "fs::copy",
        "fs::remove_",
        "fs::create_",
        "trash::delete",
    ];
    let io_patterns = [
        "std::fs::",
        "use std::fs",
        "fs::read",
        "fs::metadata",
        "fs::symlink_metadata",
        "fs::canonicalize",
        "trash::delete",
    ];

    let mut offenders = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(&source_root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if is_test_only_source(&relative) {
            continue;
        }
        let source = std::fs::read_to_string(&file).unwrap();
        let production = production_source(&source);

        for symbol in removed_symbols.iter().chain(removed_architecture.iter()) {
            if production.contains(symbol) {
                offenders.push(format!("{relative}: forbidden symbol {symbol}"));
            }
        }

        if project_io_is_allowed(&relative, &duckdb_files, &non_project_files) {
            continue;
        }
        if read_only_scanners.contains(&relative.as_str()) {
            for pattern in write_patterns {
                if let Some(offset) = production.find(pattern) {
                    let line = production[..offset].lines().count() + 1;
                    offenders.push(format!(
                        "{relative}:{line}: read-only scanner contains {pattern}"
                    ));
                }
            }
            continue;
        }
        for pattern in io_patterns {
            if production.contains(pattern) {
                offenders.push(format!("{relative}: project document I/O bypass {pattern}"));
            }
        }
    }

    assert!(!project_io_is_allowed(
        "application/unapproved_project_writer.rs",
        &duckdb_files,
        &non_project_files,
    ));
    assert!(project_io_is_allowed(
        "database/duckdb_reader.rs",
        &duckdb_files,
        &non_project_files,
    ));

    offenders.sort();
    offenders.dedup();
    assert!(
        offenders.is_empty(),
        "production project filesystem ownership violations:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn test_only_source_classification_accepts_conventional_nested_tests_rs() {
    assert!(is_test_only_source("node_system/catalog/tests.rs"));
}

#[test]
fn test_only_source_classification_rejects_similarly_named_production_file() {
    assert!(!is_test_only_source("node_system/catalog/tests_support.rs"));
}

fn project_io_is_allowed(
    relative: &str,
    duckdb_files: &[&str],
    non_project_files: &[&str],
) -> bool {
    relative.starts_with("project/filesystem/")
        || duckdb_files.contains(&relative)
        || non_project_files.contains(&relative)
}
