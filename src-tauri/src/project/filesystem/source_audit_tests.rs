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

fn production_source(source: &str) -> String {
    let marker = "#[cfg(test)]";
    let mut remaining = source;
    let mut production = String::new();
    while let Some(offset) = remaining.find(marker) {
        production.push_str(&remaining[..offset]);
        let attributed = &remaining[offset + marker.len()..];
        let item_start = attributed.len() - attributed.trim_start().len();
        let item = &attributed[item_start..];
        let semicolon = item.find(';');
        let opening = item.find('{');
        if semicolon.is_some_and(|end| opening.is_none_or(|brace| end < brace)) {
            remaining = &item[semicolon.unwrap() + 1..];
            continue;
        }
        let Some(opening) = opening else {
            break;
        };
        let mut depth = 0_u32;
        let mut item_end = None;
        for (index, byte) in item.as_bytes().iter().enumerate().skip(opening) {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        item_end = Some(index + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(item_end) = item_end else {
            break;
        };
        remaining = &item[item_end..];
    }
    production.push_str(remaining);
    production
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
        let relative = file.strip_prefix(&source_root).unwrap().to_string_lossy().replace('\\', "/");
        if relative.ends_with("_tests.rs") || relative.ends_with("production_tests.rs") {
            continue;
        }
        let source = std::fs::read_to_string(&file).unwrap();
        let production = production_source(&source);

        for symbol in removed_symbols.iter().chain(removed_architecture.iter()) {
            if production.contains(symbol) {
                offenders.push(format!("{relative}: forbidden symbol {symbol}"));
            }
        }

        if !relative.starts_with("project/") && !relative.starts_with("commands/") {
            continue;
        }
        if relative == "project/project_registry.rs" {
            continue;
        }
        if relative.starts_with("project/filesystem/") {
            continue;
        }
        if read_only_scanners.contains(&relative.as_str()) {
            for pattern in write_patterns {
                if production.contains(pattern) {
                    offenders.push(format!("{relative}: read-only scanner contains {pattern}"));
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

    offenders.sort();
    offenders.dedup();
    assert!(
        offenders.is_empty(),
        "production project filesystem ownership violations:\n{}",
        offenders.join("\n")
    );
}
