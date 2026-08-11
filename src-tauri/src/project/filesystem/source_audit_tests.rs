use std::collections::HashSet;
use std::path::{Path, PathBuf};

use regex::Regex;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Expr, FnArg, Item, Pat, PathArguments, Token, Type};

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
        "node_system/runtime/materialization.rs",
        "node_system/runtime/spill.rs",
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
    assert!(project_io_is_allowed(
        "node_system/runtime/materialization.rs",
        &duckdb_files,
        &non_project_files,
    ));
    assert!(project_io_is_allowed(
        "node_system/runtime/spill.rs",
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

fn frontend_sources(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            frontend_sources(&path, files);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "ts" || extension == "tsx")
        {
            files.push(path);
        }
    }
}

fn is_frontend_test_source(relative: &str) -> bool {
    relative.contains(".test.")
        || relative.contains(".spec.")
        || relative.starts_with("tests/")
        || relative.contains("/tests/")
        || relative.contains("/__tests__/")
}

struct TypeScriptFunction<'a> {
    name: &'a str,
    parameters: &'a str,
    body: &'a str,
}

fn matching_delimiter(source: &str, start: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.get(start) != Some(&open) {
        return None;
    }
    let mut depth = 0_u32;
    let mut quote = None;
    let mut index = start;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active_quote) = quote {
            if byte == b'\\' {
                index += 1;
            } else if byte == active_quote {
                quote = None;
            }
        } else if matches!(byte, b'\'' | b'"' | b'`') {
            quote = Some(byte);
        } else if byte == open {
            depth += 1;
        } else if byte == close {
            depth -= 1;
            if depth == 0 {
                return Some(index + 1);
            }
        }
        index += 1;
    }
    None
}

fn skip_ascii_whitespace(source: &str, mut index: usize) -> usize {
    while source
        .as_bytes()
        .get(index)
        .is_some_and(u8::is_ascii_whitespace)
    {
        index += 1;
    }
    index
}

fn typescript_expression_end(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut delimiters = Vec::new();
    let mut quote = None;
    let mut index = start;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active_quote) = quote {
            if byte == b'\\' {
                index += 1;
            } else if byte == active_quote {
                quote = None;
            }
        } else {
            match byte {
                b'\'' | b'"' | b'`' => quote = Some(byte),
                b'(' => delimiters.push(b')'),
                b'[' => delimiters.push(b']'),
                b'{' => delimiters.push(b'}'),
                b')' | b']' | b'}' if delimiters.last() == Some(&byte) => {
                    delimiters.pop();
                }
                b';' | b'\n' | b'\r' if delimiters.is_empty() => break,
                _ => {}
            }
        }
        index += 1;
    }
    index
}

fn assigned_arrow_functions(source: &str) -> Vec<TypeScriptFunction<'_>> {
    let assignment = Regex::new(
        r"(?m)\b(?:const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)(?:\s*:[^=\r\n]+)?\s*=\s*(?:async\s*)?",
    )
    .unwrap();
    assignment
        .captures_iter(source)
        .filter_map(|captures| {
            let name = captures.get(1)?.as_str();
            let mut cursor = captures.get(0)?.end();
            let (parameters, after_parameters) = if source.as_bytes().get(cursor) == Some(&b'(') {
                let end = matching_delimiter(source, cursor, b'(', b')')?;
                (&source[cursor + 1..end - 1], end)
            } else {
                let parameter = Regex::new(r"^[A-Za-z_$][A-Za-z0-9_$]*")
                    .unwrap()
                    .find(&source[cursor..])?;
                cursor += parameter.end();
                (parameter.as_str(), cursor)
            };
            cursor = skip_ascii_whitespace(source, after_parameters);
            if !source[cursor..].starts_with("=>") {
                let arrow = source[cursor..].find("=>")?;
                let annotation = &source[cursor..cursor + arrow];
                if annotation.contains([';', '\n', '\r']) {
                    return None;
                }
                cursor += arrow;
            }
            cursor += 2;
            cursor = skip_ascii_whitespace(source, cursor);
            let body_end = if source.as_bytes().get(cursor) == Some(&b'{') {
                matching_delimiter(source, cursor, b'{', b'}')?
            } else {
                typescript_expression_end(source, cursor)
            };
            Some(TypeScriptFunction {
                name,
                parameters,
                body: &source[cursor..body_end],
            })
        })
        .collect()
}

fn typescript_functions(source: &str) -> Vec<TypeScriptFunction<'_>> {
    let declarations = Regex::new(
        r"(?m)\bfunction\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*\(([^)]*)\)(?:\s*:\s*[^\{\r\n]+)?\s*\{",
    )
    .unwrap();
    let mut functions = declarations
        .captures_iter(source)
        .filter_map(|captures| {
            let matched = captures.get(0)?;
            let body_start = matched.end() - 1;
            let body_end = matching_delimiter(source, body_start, b'{', b'}')?;
            Some(TypeScriptFunction {
                name: captures.get(1)?.as_str(),
                parameters: captures.get(2)?.as_str(),
                body: &source[body_start..body_end],
            })
        })
        .collect::<Vec<_>>();
    functions.extend(assigned_arrow_functions(source));
    functions
}

fn worksheet_path_binding(name: &str) -> bool {
    name.chars()
        .filter(|character| *character != '_' && *character != '$')
        .collect::<String>()
        .eq_ignore_ascii_case("worksheetpath")
}

fn worksheet_name_extractor_api(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("worksheet")
        && [
            "label", "name", "path", "basename", "display", "extract", "parse",
        ]
        .iter()
        .any(|semantic| lower.contains(semantic))
}

struct TypeScriptBinding {
    source: Option<String>,
    destination: String,
}

fn leading_identifier(value: &str) -> Option<String> {
    Regex::new(r"^[A-Za-z_$][A-Za-z0-9_$]*")
        .unwrap()
        .find(value.trim())
        .map(|matched| matched.as_str().to_owned())
}

fn binding_pattern(pattern: &str) -> Vec<TypeScriptBinding> {
    let pattern = pattern.trim();
    if pattern.starts_with('{') {
        let end = pattern.find('}').unwrap_or(pattern.len());
        return pattern[1..end]
            .split(',')
            .filter_map(|property| {
                let property = property.trim().trim_start_matches("...");
                let (source, destination) = property
                    .split_once(':')
                    .map_or((property, property), |(source, destination)| {
                        (source, destination)
                    });
                Some(TypeScriptBinding {
                    source: leading_identifier(source),
                    destination: leading_identifier(destination)?,
                })
            })
            .collect();
    }
    if pattern.starts_with('[') {
        let end = pattern.find(']').unwrap_or(pattern.len());
        return pattern[1..end]
            .split(',')
            .filter_map(|binding| {
                leading_identifier(binding.trim().trim_start_matches("...")).map(|destination| {
                    TypeScriptBinding {
                        source: None,
                        destination,
                    }
                })
            })
            .collect();
    }
    leading_identifier(pattern)
        .map(|destination| {
            vec![TypeScriptBinding {
                source: None,
                destination,
            }]
        })
        .unwrap_or_default()
}

fn parameter_bindings(parameters: &str) -> Vec<TypeScriptBinding> {
    let mut bindings = Vec::new();
    let mut start = 0;
    let mut depth = 0_i32;
    for (index, character) in parameters.char_indices() {
        match character {
            '{' | '[' | '(' | '<' => depth += 1,
            '}' | ']' | ')' | '>' => depth -= 1,
            ',' if depth == 0 => {
                bindings.extend(binding_pattern(&parameters[start..index]));
                start = index + 1;
            }
            _ => {}
        }
    }
    bindings.extend(binding_pattern(&parameters[start..]));
    bindings
}

fn expression_is_tainted_alias(expression: &str, tainted: &HashSet<String>) -> bool {
    let expression = expression
        .trim()
        .trim_matches(|character| character == '(' || character == ')');
    tainted.iter().any(|binding| {
        expression == binding
            || expression
                .strip_prefix(binding)
                .is_some_and(|suffix| suffix.trim_start().starts_with('.'))
    })
}

fn function_extracts_tainted_path(function: &TypeScriptFunction<'_>) -> bool {
    let semantic_api = worksheet_name_extractor_api(function.name);
    let mut tainted = parameter_bindings(function.parameters)
        .into_iter()
        .filter(|binding| {
            semantic_api
                || worksheet_path_binding(&binding.destination)
                || binding
                    .source
                    .as_deref()
                    .is_some_and(worksheet_path_binding)
        })
        .map(|binding| binding.destination)
        .collect::<HashSet<_>>();
    let declarations =
        Regex::new(r"(?m)\b(?:const|let|var)\s+([^=;\r\n]+?)\s*=\s*([^;\r\n]+)").unwrap();
    loop {
        let mut changed = false;
        for captures in declarations.captures_iter(function.body) {
            let bindings = binding_pattern(captures.get(1).unwrap().as_str());
            let aliases_taint =
                expression_is_tainted_alias(captures.get(2).unwrap().as_str(), &tainted);
            for binding in bindings {
                let source_is_worksheet_path = binding
                    .source
                    .as_deref()
                    .is_some_and(worksheet_path_binding);
                if (aliases_taint
                    || source_is_worksheet_path
                    || worksheet_path_binding(&binding.destination))
                    && tainted.insert(binding.destination)
                {
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    tainted.iter().any(|binding| {
        let binding = regex::escape(binding);
        let receiver_operation = Regex::new(&format!(
            r"\b{binding}\s*\.\s*(?:split|lastIndexOf|substring|substr|slice|replace|replaceAll|match)\s*\("
        ))
        .unwrap();
        let path_api = Regex::new(&format!(
            r"\b(?:[A-Za-z_$][A-Za-z0-9_$]*\s*\.\s*)?(?:basename|extname|dirname)\s*\(\s*{binding}\b"
        ))
        .unwrap();
        receiver_operation.is_match(function.body) || path_api.is_match(function.body)
    })
}

fn frontend_worksheet_path_parser_violations(sources: &[(&str, &str)]) -> Vec<String> {
    let mut violations = Vec::new();
    for (path, source) in sources {
        for function in typescript_functions(source) {
            if function_extracts_tainted_path(&function) {
                violations.push(format!(
                    "frontend/{path}: forbidden worksheet path parsing in {}",
                    function.name
                ));
            }
        }
    }
    violations
}

fn type_is_uuid(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Uuid"),
        Type::Reference(reference) => type_is_uuid(&reference.elem),
        Type::Paren(paren) => type_is_uuid(&paren.elem),
        Type::Group(group) => type_is_uuid(&group.elem),
        _ => false,
    }
}

fn rust_pattern_bindings(pattern: &Pat, bindings: &mut Vec<String>) {
    match pattern {
        Pat::Ident(ident) => bindings.push(ident.ident.to_string()),
        Pat::Type(typed) => rust_pattern_bindings(&typed.pat, bindings),
        Pat::Tuple(tuple) => {
            for element in &tuple.elems {
                rust_pattern_bindings(element, bindings);
            }
        }
        Pat::TupleStruct(tuple) => {
            for element in &tuple.elems {
                rust_pattern_bindings(element, bindings);
            }
        }
        Pat::Struct(structure) => {
            for field in &structure.fields {
                rust_pattern_bindings(&field.pat, bindings);
            }
        }
        Pat::Reference(reference) => rust_pattern_bindings(&reference.pat, bindings),
        Pat::Slice(slice) => {
            for element in &slice.elems {
                rust_pattern_bindings(element, bindings);
            }
        }
        _ => {}
    }
}

fn pattern_has_uuid_type(pattern: &Pat) -> bool {
    matches!(pattern, Pat::Type(typed) if type_is_uuid(&typed.ty))
}

#[derive(Default)]
struct UuidWorksheetFilenameVisitor {
    uuid_bindings: Vec<HashSet<String>>,
    violation: bool,
}

impl UuidWorksheetFilenameVisitor {
    fn active_bindings(&self) -> Option<&HashSet<String>> {
        self.uuid_bindings.last()
    }

    fn bindings_for_inputs(inputs: &Punctuated<FnArg, Token![,]>) -> HashSet<String> {
        let mut bindings = HashSet::new();
        for argument in inputs {
            let FnArg::Typed(argument) = argument else {
                continue;
            };
            let mut names = Vec::new();
            rust_pattern_bindings(&argument.pat, &mut names);
            if type_is_uuid(&argument.ty)
                || names
                    .iter()
                    .any(|name| name.to_ascii_lowercase().contains("uuid"))
            {
                bindings.extend(names);
            }
        }
        bindings
    }

    fn visit_function_scope(&mut self, signature: &syn::Signature, block: &syn::Block) {
        self.uuid_bindings
            .push(Self::bindings_for_inputs(&signature.inputs));
        visit::visit_block(self, block);
        self.uuid_bindings.pop();
    }

    fn expression_is_uuid_backed(&self, expression: &Expr) -> bool {
        match expression {
            Expr::Path(path) => path
                .path
                .get_ident()
                .zip(self.active_bindings())
                .is_some_and(|(ident, bindings)| bindings.contains(&ident.to_string())),
            Expr::Call(call) => {
                let uuid_constructor = matches!(call.func.as_ref(), Expr::Path(path)
                    if path.path.segments.iter().any(|segment| segment.ident == "Uuid"));
                uuid_constructor
                    || call
                        .args
                        .iter()
                        .any(|argument| self.expression_is_uuid_backed(argument))
            }
            Expr::MethodCall(call) => {
                let parses_uuid = call.turbofish.as_ref().is_some_and(|arguments| {
                    arguments.args.iter().any(|argument| {
                        matches!(argument, syn::GenericArgument::Type(ty) if type_is_uuid(ty))
                    })
                });
                parses_uuid || self.expression_is_uuid_backed(&call.receiver)
            }
            Expr::Reference(reference) => self.expression_is_uuid_backed(&reference.expr),
            Expr::Paren(paren) => self.expression_is_uuid_backed(&paren.expr),
            Expr::Group(group) => self.expression_is_uuid_backed(&group.expr),
            Expr::Assign(assign) => self.expression_is_uuid_backed(&assign.right),
            _ => false,
        }
    }

    fn format_builds_uuid_worksheet_filename(&self, mac: &syn::Macro) -> bool {
        if !mac
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "format")
        {
            return false;
        }
        let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
        let Ok(arguments) = parser.parse2(mac.tokens.clone()) else {
            return false;
        };
        let mut arguments = arguments.iter();
        let Some(Expr::Lit(template)) = arguments.next() else {
            return false;
        };
        let syn::Lit::Str(template) = &template.lit else {
            return false;
        };
        if !template.value().contains(".yssbi-worksheet") {
            return false;
        }
        let explicit_uuid_argument =
            arguments.any(|argument| self.expression_is_uuid_backed(argument));
        let implicit_uuid_binding = Regex::new(r"\{([A-Za-z_][A-Za-z0-9_]*)(?::[^}]*)?\}")
            .unwrap()
            .captures_iter(&template.value())
            .filter_map(|captures| captures.get(1))
            .any(|binding| {
                self.active_bindings()
                    .is_some_and(|bindings| bindings.contains(binding.as_str()))
            });
        explicit_uuid_argument || implicit_uuid_binding
    }
}

fn has_cfg_test(attributes: &[syn::Attribute]) -> bool {
    let test_token = Regex::new(r"\btest\b").unwrap();
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && matches!(&attribute.meta, syn::Meta::List(list) if test_token.is_match(&list.tokens.to_string()))
    })
}

impl<'ast> Visit<'ast> for UuidWorksheetFilenameVisitor {
    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if !has_cfg_test(&module.attrs) {
            visit::visit_item_mod(self, module);
        }
    }

    fn visit_item_impl(&mut self, implementation: &'ast syn::ItemImpl) {
        if !has_cfg_test(&implementation.attrs) {
            visit::visit_item_impl(self, implementation);
        }
    }

    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        if !has_cfg_test(&function.attrs) {
            self.visit_function_scope(&function.sig, &function.block);
        }
    }

    fn visit_impl_item_fn(&mut self, function: &'ast syn::ImplItemFn) {
        if !has_cfg_test(&function.attrs) {
            self.visit_function_scope(&function.sig, &function.block);
        }
    }

    fn visit_trait_item_fn(&mut self, function: &'ast syn::TraitItemFn) {
        if has_cfg_test(&function.attrs) {
            return;
        }
        if let Some(default) = &function.default {
            self.visit_function_scope(&function.sig, default);
        }
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        let mut names = Vec::new();
        rust_pattern_bindings(&local.pat, &mut names);
        let explicit_uuid_name = names
            .iter()
            .any(|name| name.to_ascii_lowercase().contains("uuid"));
        let uuid_backed = pattern_has_uuid_type(&local.pat)
            || explicit_uuid_name
            || local
                .init
                .as_ref()
                .is_some_and(|init| self.expression_is_uuid_backed(&init.expr));
        if uuid_backed {
            if let Some(bindings) = self.uuid_bindings.last_mut() {
                bindings.extend(names);
            }
        }
        visit::visit_local(self, local);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.violation |= self.format_builds_uuid_worksheet_filename(mac);
        visit::visit_macro(self, mac);
    }
}

fn rust_formatted_uuid_worksheet_violations(sources: &[(&str, &str)]) -> Vec<String> {
    let mut violations = Vec::new();
    for (path, source) in sources {
        let syntax = syn::parse_file(source)
            .unwrap_or_else(|error| panic!("failed to parse Rust audit fixture {path}: {error}"));
        let mut visitor = UuidWorksheetFilenameVisitor::default();
        visitor.visit_file(&syntax);
        if visitor.violation {
            violations.push(format!(
                "rust/{path}: forbidden formatted UUID worksheet filename"
            ));
        }
    }
    violations
}

#[test]
fn worksheet_architecture_audit_rejects_aliased_path_parsing() {
    let sources = [(
        "src/features/worksheetLabel.ts",
        "function worksheetLabel(path: string) { return path.split('/').pop(); }",
    )];

    assert!(!frontend_worksheet_path_parser_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_rejects_arrow_function_alias_flow() {
    let sources = [(
        "src/features/worksheetLabel.ts",
        r#"
            const worksheetLabel = (input: string) => {
                const path = input;
                return path.split('/').pop();
            };
        "#,
    )];

    assert!(!frontend_worksheet_path_parser_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_rejects_destructured_worksheet_path_flow() {
    let sources = [(
        "src/features/displayLabel.ts",
        r#"
            const displayLabel = ({ worksheetPath }: { worksheetPath: string }) => {
                const path = worksheetPath;
                return basename(path);
            };
        "#,
    )];

    assert!(!frontend_worksheet_path_parser_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_rejects_renamed_worksheet_path_destructuring() {
    let sources = [(
        "src/features/displayLabel.ts",
        r#"
            const displayLabel = ({ worksheetPath: path }: { worksheetPath: string }) => {
                return path.split('/').pop();
            };
        "#,
    )];

    assert!(!frontend_worksheet_path_parser_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_allows_unrelated_renamed_destructuring() {
    let sources = [(
        "src/features/displayLabel.ts",
        r#"
            const displayLabel = ({ csv: path }: { csv: string }) => {
                return path.split('/');
            };
        "#,
    )];

    assert!(frontend_worksheet_path_parser_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_rejects_unparenthesized_expression_arrow() {
    let sources = [(
        "src/features/worksheetLabel.ts",
        "const worksheetLabel = path => path.split('/').pop();",
    )];

    assert!(!frontend_worksheet_path_parser_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_rejects_typed_expression_arrow() {
    let sources = [(
        "src/features/worksheetLabel.ts",
        "const worksheetLabel = (path: string) => path.split('/').pop();",
    )];

    assert!(!frontend_worksheet_path_parser_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_allows_unrelated_expression_arrows() {
    let sources = [
        (
            "src/features/plainRows.ts",
            "const plainRows = csv => csv.split('/');",
        ),
        (
            "src/features/worksheetRows.ts",
            "const worksheetRows = (csv: string) => csv.split('/');",
        ),
    ];

    assert!(frontend_worksheet_path_parser_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_allows_unrelated_worksheet_rows_split() {
    let sources = [(
        "src/features/worksheetRows.ts",
        "function worksheetRows(csv: string) { return csv.split('/'); }",
    )];

    assert!(frontend_worksheet_path_parser_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_allows_opaque_path_pass_through() {
    let sources = [(
        "src/features/forwardWorksheet.ts",
        "function forwardWorksheet(path: string) { return consumeOpaquePath(path); }",
    )];

    assert!(frontend_worksheet_path_parser_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_rejects_formatted_uuid_filenames() {
    let sources = [(
        "src/project/legacy.rs",
        r#"
            use uuid::Uuid;

            fn legacy_worksheet_path() {
                let worksheet_id: Uuid = Uuid::new_v4();
                let path = format!("worksheets/{worksheet_id}.yssbi-worksheet");
            }
        "#,
    )];

    assert!(!rust_formatted_uuid_worksheet_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_tracks_uuid_parse_alias_into_path_join() {
    let sources = [(
        "src/project/legacy.rs",
        r#"
            use uuid::Uuid;

            fn legacy_worksheet_path(raw: &str) {
                let parsed_uuid = Uuid::parse_str(raw).unwrap();
                let id = parsed_uuid;
                let path = Path::new("worksheets")
                    .join(format!("{id}.yssbi-worksheet"));
            }
        "#,
    )];

    assert!(!rust_formatted_uuid_worksheet_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_tracks_explicit_uuid_semantic_binding() {
    let sources = [(
        "src/project/legacy.rs",
        r#"
            fn legacy_worksheet_path(worksheet_uuid: &str) {
                let path = format!("worksheets/{worksheet_uuid}.yssbi-worksheet");
            }
        "#,
    )];

    assert!(!rust_formatted_uuid_worksheet_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_rejects_uuid_filename_inside_impl_method() {
    let sources = [(
        "src/project/legacy.rs",
        r#"
            impl LegacyWorksheetStore {
                fn worksheet_path(&self, worksheet_id: Uuid) -> String {
                    let id = worksheet_id;
                    format!("worksheets/{id}.yssbi-worksheet")
                }
            }
        "#,
    )];

    assert!(!rust_formatted_uuid_worksheet_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_rejects_uuid_filename_inside_trait_default_method() {
    let sources = [(
        "src/project/legacy.rs",
        r#"
            trait LegacyWorksheetPath {
                fn worksheet_path(&self, worksheet_id: Uuid) -> String {
                    let id = worksheet_id;
                    format!("worksheets/{id}.yssbi-worksheet")
                }
            }
        "#,
    )];

    assert!(!rust_formatted_uuid_worksheet_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_allows_resource_name_inside_impl_method() {
    let sources = [(
        "src/project/resource_path.rs",
        r#"
            impl WorksheetStore {
                fn worksheet_path(&self, name: ResourceName) -> String {
                    let validated_name = name;
                    format!("worksheets/{validated_name}.yssbi-worksheet")
                }
            }
        "#,
    )];

    assert!(rust_formatted_uuid_worksheet_violations(&sources).is_empty());
}

#[test]
fn worksheet_architecture_audit_allows_resource_name_filenames() {
    let sources = [(
        "src/project/resource_path.rs",
        r#"
            fn worksheet_path() {
                let name = ResourceName::parse("Sales").unwrap();
                let path = format!("worksheets/{name}.yssbi-worksheet");
            }
        "#,
    )];

    assert!(rust_formatted_uuid_worksheet_violations(&sources).is_empty());
}

#[test]
fn production_worksheet_contract_has_no_legacy_identity_or_layering_bypasses() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rust_root = manifest.join("src");
    let frontend_root = manifest.parent().unwrap().join("src");
    let forbidden_identity = ["worksheetId", "WorksheetDeltaDto", "worksheetDeltas"];
    let uuid_worksheet = Regex::new(
        r"worksheets/[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\.yssbi-worksheet",
    )
    .unwrap();
    let mut offenders = Vec::new();

    let mut rust_files = Vec::new();
    rust_sources(&rust_root, &mut rust_files);
    for file in rust_files {
        let relative = file
            .strip_prefix(&rust_root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if is_test_only_source(&relative) {
            continue;
        }
        let full_source = std::fs::read_to_string(&file).unwrap();
        let source = production_source(&full_source);
        for forbidden in forbidden_identity {
            if source.contains(forbidden) {
                offenders.push(format!(
                    "rust/{relative}: forbidden worksheet contract {forbidden}"
                ));
            }
        }
        if uuid_worksheet.is_match(&source) {
            offenders.push(format!(
                "rust/{relative}: forbidden UUID worksheet filename contract"
            ));
        }
        offenders.extend(rust_formatted_uuid_worksheet_violations(&[(
            relative.as_str(),
            full_source.as_str(),
        )]));
    }

    let mut frontend_files = Vec::new();
    frontend_sources(&frontend_root, &mut frontend_files);
    for file in frontend_files {
        let relative = file
            .strip_prefix(&frontend_root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if is_frontend_test_source(&relative) {
            continue;
        }
        let source = std::fs::read_to_string(&file).unwrap();
        for forbidden in forbidden_identity {
            if source.contains(forbidden) {
                offenders.push(format!(
                    "frontend/{relative}: forbidden worksheet contract {forbidden}"
                ));
            }
        }
        if uuid_worksheet.is_match(&source) {
            offenders.push(format!(
                "frontend/{relative}: forbidden UUID worksheet filename contract"
            ));
        }
        offenders.extend(frontend_worksheet_path_parser_violations(&[(
            relative.as_str(),
            source.as_str(),
        )]));

        if relative.starts_with("views/")
            && (source.contains("@tauri-apps/api/core")
                || Regex::new(r"\binvoke\s*\(").unwrap().is_match(&source))
        {
            offenders.push(format!(
                "frontend/{relative}: direct view-layer Tauri invoke"
            ));
        }
    }

    offenders.sort();
    assert!(
        offenders.is_empty(),
        "production worksheet architecture violations:\n{}",
        offenders.join("\n")
    );
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
