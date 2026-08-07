use std::path::{Path, PathBuf};

use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{FnArg, Item, Pat, PathArguments, Token, Type};

const IDENTITY_REQUIRED_TAURI_COMMANDS: &[&str] = &[
    "mutate_graph_document",
    "update_function_signature",
    "hydrate_editor_graph",
    "get_project_history_status",
    "undo_graph_document",
    "redo_graph_document",
    "execute_graph_document",
    "load_database",
    "delete_database",
    "rename_database",
    "get_database_meta",
    "get_database_rows",
    "get_column_stats",
    "get_column_distribution",
    "get_dataset_overview",
    "edit_cell",
    "add_row",
    "delete_rows",
    "add_column",
    "delete_column",
    "cast_column",
    "rename_column",
    "undo_edit",
    "redo_edit",
    "save_database_changes",
    "export_database",
    "get_edit_state",
    "get_plot_column_pair",
];

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
    relative.starts_with("node_system/testing/")
        || relative.rsplit('/').next() == Some("tests.rs")
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
fn test_only_source_classification_accepts_cfg_test_module_trees() {
    assert!(is_test_only_source("node_system/testing/contracts.rs"));
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

#[derive(Debug)]
struct TauriCommandDefinition {
    path: String,
    name: String,
    has_project_instance_id: bool,
}

fn has_tauri_command_attribute(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        let mut segments = attribute.path().segments.iter();
        segments
            .next()
            .is_some_and(|segment| segment.ident == "tauri")
            && segments
                .next()
                .is_some_and(|segment| segment.ident == "command")
            && segments.next().is_none()
    })
}

fn is_exact_project_instance_id(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path.path.segments.len() == 1
        && path.path.segments[0].ident == "ProjectInstanceId"
        && matches!(path.path.segments[0].arguments, PathArguments::None)
}

fn has_project_instance_id_parameter(function: &syn::ItemFn) -> bool {
    function.sig.inputs.iter().any(|argument| {
        let FnArg::Typed(argument) = argument else {
            return false;
        };
        matches!(argument.pat.as_ref(), Pat::Ident(ident) if ident.ident == "project_instance_id")
            && is_exact_project_instance_id(argument.ty.as_ref())
    })
}

fn collect_tauri_commands(
    path: &str,
    items: &[Item],
    definitions: &mut Vec<TauriCommandDefinition>,
) {
    for item in items {
        match item {
            Item::Fn(function) if has_tauri_command_attribute(&function.attrs) => {
                definitions.push(TauriCommandDefinition {
                    path: path.to_owned(),
                    name: function.sig.ident.to_string(),
                    has_project_instance_id: has_project_instance_id_parameter(function),
                });
            }
            Item::Mod(module) => {
                if let Some((_, items)) = &module.content {
                    collect_tauri_commands(path, items, definitions);
                }
            }
            _ => {}
        }
    }
}

fn tauri_command_identity_violations(
    sources: &[(&str, &str)],
    audited_commands: &[&str],
) -> Vec<String> {
    let mut definitions = Vec::new();
    let mut violations = Vec::new();
    for (path, source) in sources {
        match syn::parse_file(source) {
            Ok(file) => collect_tauri_commands(path, &file.items, &mut definitions),
            Err(error) => violations.push(format!("{path}: Rust syntax parse failed: {error}")),
        }
    }
    for command in audited_commands {
        let matches = definitions
            .iter()
            .filter(|definition| definition.name == *command)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => violations.push(format!("missing Rust command signature: {command}")),
            [definition] if !definition.has_project_instance_id => violations.push(format!(
                "{}: {command} missing required project_instance_id",
                definition.path
            )),
            [_] => {}
            _ => violations.push(format!(
                "ambiguous Rust command signature: {command} ({} definitions)",
                matches.len()
            )),
        }
    }
    violations
}

#[derive(Default)]
struct GenerateHandlerVisitor {
    commands: Vec<String>,
}

impl Visit<'_> for GenerateHandlerVisitor {
    fn visit_macro(&mut self, node: &syn::Macro) {
        if node
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "generate_handler")
        {
            let parser = Punctuated::<syn::Path, Token![,]>::parse_terminated;
            if let Ok(paths) = parser.parse2(node.tokens.clone()) {
                self.commands.extend(paths.into_iter().filter_map(|path| {
                    path.segments
                        .last()
                        .map(|segment| segment.ident.to_string())
                }));
            }
        }
        visit::visit_macro(self, node);
    }
}

fn registered_tauri_commands(source: &str) -> Vec<String> {
    let syntax = syn::parse_file(source).expect("src/lib.rs must be valid Rust syntax");
    let mut visitor = GenerateHandlerVisitor::default();
    visitor.visit_file(&syntax);
    visitor.commands
}

#[test]
fn identity_required_tauri_commands_are_registered_once_and_have_unique_typed_definitions() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let command_root = manifest.join("src/commands");
    let mut files = Vec::new();
    rust_sources(&command_root, &mut files);
    let owned_sources = files
        .iter()
        .map(|path| {
            (
                path.strip_prefix(manifest)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                std::fs::read_to_string(path).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let sources = owned_sources
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect::<Vec<_>>();
    let registered =
        registered_tauri_commands(&std::fs::read_to_string(manifest.join("src/lib.rs")).unwrap());
    let registration_violations = IDENTITY_REQUIRED_TAURI_COMMANDS
        .iter()
        .filter_map(|command| {
            let count = registered
                .iter()
                .filter(|registered| *registered == command)
                .count();
            (count != 1).then(|| format!("{command}: expected one registration, found {count}"))
        })
        .collect::<Vec<_>>();

    assert!(
        registration_violations.is_empty(),
        "identity command registration violations:\n{}",
        registration_violations.join("\n")
    );
    let violations = tauri_command_identity_violations(&sources, IDENTITY_REQUIRED_TAURI_COMMANDS);
    assert!(
        violations.is_empty(),
        "identity command definition violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn tauri_command_identity_audit_ignores_comment_string_and_non_command_decoys() {
    let fixtures = [
        (
            "block_comment.rs",
            r#"
                /*
                #[tauri::command]
                pub fn get_database_rows(project_instance_id: ProjectInstanceId) {}
                */
                #[tauri::command]
                pub fn get_database_rows(id: String) {}
            "#,
        ),
        (
            "raw_string.rs",
            r##"
                const DECOY: &str = r#"
                    #[tauri::command]
                    pub fn edit_cell(project_instance_id: ProjectInstanceId) {}
                "#;
                #[tauri::command]
                pub fn edit_cell(id: String) {}
            "##,
        ),
        (
            "non_command.rs",
            r#"
                pub fn get_database_meta(project_instance_id: ProjectInstanceId) {}
                #[tauri::command]
                pub fn get_database_meta(id: String) {}
            "#,
        ),
    ];

    assert_eq!(
        tauri_command_identity_violations(
            &fixtures,
            &["get_database_rows", "edit_cell", "get_database_meta"],
        ),
        vec![
            "block_comment.rs: get_database_rows missing required project_instance_id",
            "raw_string.rs: edit_cell missing required project_instance_id",
            "non_command.rs: get_database_meta missing required project_instance_id",
        ],
    );
}

#[test]
fn tauri_command_identity_audit_ignores_commented_parameters() {
    let fixtures = [(
        "commented_parameter.rs",
        r#"
            #[tauri::command]
            pub fn get_database_rows(
                // project_instance_id: ProjectInstanceId,
                id: String,
            ) {}
        "#,
    )];

    assert_eq!(
        tauri_command_identity_violations(&fixtures, &["get_database_rows"]),
        vec!["commented_parameter.rs: get_database_rows missing required project_instance_id"],
    );
}

#[test]
fn tauri_command_identity_audit_rejects_duplicate_actual_commands() {
    let fixtures = [
        (
            "first.rs",
            "#[tauri::command]\npub fn get_database_rows(project_instance_id: ProjectInstanceId) {}",
        ),
        (
            "second.rs",
            "#[tauri::command]\npub fn get_database_rows(project_instance_id: ProjectInstanceId) {}",
        ),
    ];

    assert_eq!(
        tauri_command_identity_violations(&fixtures, &["get_database_rows"]),
        vec!["ambiguous Rust command signature: get_database_rows (2 definitions)"],
    );
}
