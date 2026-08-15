use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::graph_resource_path::{GraphResourcePath, normalize_graph_resource_path};
use super::project_error::ProjectError;
use super::{
    EVENT_EXTENSION, EVENTS_DIR, FUNCTION_EXTENSION, FUNCTIONS_DIR, GraphDocumentKind, ResourceName,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedGraphEntry {
    pub path: GraphResourcePath,
    pub kind: GraphDocumentKind,
}

#[derive(Debug, Clone)]
pub struct GraphResourceIndex {
    entries: Vec<ScannedGraphEntry>,
    by_path: HashMap<String, ScannedGraphEntry>,
}

impl GraphResourceIndex {
    pub fn entries(&self) -> &[ScannedGraphEntry] {
        &self.entries
    }

    pub fn get_by_path(&self, path: &str) -> Option<&ScannedGraphEntry> {
        self.by_path.get(&normalize_resource_path(path))
    }
}

/// 扫描 `events/` 与 `functions/` 目录，路径为唯一身份。
pub fn scan_graph_resource_index(root: &Path) -> Result<GraphResourceIndex, ProjectError> {
    let files = collect_graph_resource_files(root)?;
    let mut entries = Vec::with_capacity(files.len());
    let mut portable_paths = HashSet::new();
    for (path, kind) in files {
        let path = GraphResourcePath::new(path)?;
        let name = ResourceName::parse(path.display_name()).map_err(|error| {
            ProjectError::InvalidProjectFormat(format!(
                "invalid graph resource name in '{}': {error}",
                path.as_str()
            ))
        })?;
        let portable_path = format!("{kind:?}:{}", name.portable_key());
        if !portable_paths.insert(portable_path) {
            return Err(ProjectError::InvalidProjectFormat(format!(
                "portable graph path collision at '{}'",
                path.as_str()
            )));
        }
        entries.push(ScannedGraphEntry { path, kind });
    }
    entries.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));

    let by_path = entries
        .iter()
        .cloned()
        .map(|entry| (entry.path.as_str().to_string(), entry))
        .collect();

    Ok(GraphResourceIndex { entries, by_path })
}

pub fn normalize_resource_path(path: &str) -> String {
    normalize_graph_resource_path(path)
}

fn collect_graph_resource_files(
    root: &Path,
) -> Result<Vec<(String, GraphDocumentKind)>, ProjectError> {
    let mut files = Vec::new();
    collect_kind_files(
        root,
        EVENTS_DIR,
        EVENT_EXTENSION,
        GraphDocumentKind::Event,
        &mut files,
    )?;
    collect_kind_files(
        root,
        FUNCTIONS_DIR,
        FUNCTION_EXTENSION,
        GraphDocumentKind::Function,
        &mut files,
    )?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

fn collect_kind_files(
    root: &Path,
    dir: &str,
    extension: &str,
    kind: GraphDocumentKind,
    files: &mut Vec<(String, GraphDocumentKind)>,
) -> Result<(), ProjectError> {
    let graph_dir = root.join(dir);
    let metadata = match std::fs::symlink_metadata(&graph_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if crate::project::metadata_is_redirect(&metadata) || !metadata.is_dir() {
        return Err(ProjectError::InvalidProjectFormat(format!(
            "graph directory '{}' is not a real directory",
            graph_dir.display()
        )));
    }
    for entry in std::fs::read_dir(&graph_dir)? {
        let path = entry?.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if crate::project::metadata_is_redirect(&metadata) {
            return Err(ProjectError::InvalidProjectFormat(format!(
                "graph resource path '{}' is a redirect",
                path.display()
            )));
        }
        if metadata.is_dir() {
            return Err(ProjectError::InvalidProjectFormat(format!(
                "nested graph directories are not supported: '{}'",
                path.display()
            )));
        }
        if is_graph_file(&path, &metadata, extension) {
            files.push((relative_slash_path(root, &path)?, kind));
        }
    }
    Ok(())
}

fn is_graph_file(path: &Path, metadata: &std::fs::Metadata, extension: &str) -> bool {
    metadata.is_file()
        && path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case(extension))
            .unwrap_or(false)
}

fn relative_slash_path(root: &Path, path: &Path) -> Result<String, ProjectError> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::scan_graph_resource_index;

    struct TestTree {
        root: std::path::PathBuf,
        outside: std::path::PathBuf,
    }

    impl TestTree {
        fn new(label: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "yssbi-graph-discovery-{label}-{}",
                uuid::Uuid::new_v4()
            ));
            let root = base.join("project");
            let outside = base.join("outside");
            std::fs::create_dir_all(root.join("events")).unwrap();
            std::fs::create_dir_all(&outside).unwrap();
            Self { root, outside }
        }
    }

    impl Drop for TestTree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.root.parent().expect("test project has a parent"));
        }
    }

    #[cfg(windows)]
    fn link_directory(link: &std::path::Path, target: &std::path::Path) {
        let command = format!(
            "New-Item -ItemType Junction -Path '{}' -Target '{}' | Out-Null",
            link.display(),
            target.display()
        );
        let status = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                command.as_str(),
            ])
            .status()
            .unwrap();
        assert!(status.success(), "failed to create test junction");
    }

    #[cfg(unix)]
    fn link_directory(link: &std::path::Path, target: &std::path::Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    #[cfg(windows)]
    fn try_link_file(link: &std::path::Path, target: &std::path::Path) -> bool {
        match std::os::windows::fs::symlink_file(target, link) {
            Ok(()) => true,
            Err(error)
                if error.raw_os_error() == Some(1314)
                    || error.kind() == std::io::ErrorKind::Unsupported =>
            {
                eprintln!("skipping test: Windows file symlinks are unavailable: {error}");
                false
            }
            Err(error) => panic!("failed to create test file symlink: {error}"),
        }
    }

    #[cfg(unix)]
    fn try_link_file(link: &std::path::Path, target: &std::path::Path) -> bool {
        std::os::unix::fs::symlink(target, link).unwrap();
        true
    }

    #[test]
    fn graph_discovery_rejects_portable_casefold_collisions() {
        let tree = TestTree::new("casefold-collision");
        std::fs::write(tree.root.join("events/Straße.yssbi-event"), b"{}").unwrap();
        std::fs::write(tree.root.join("events/STRASSE.yssbi-event"), b"{}").unwrap();

        let error = scan_graph_resource_index(&tree.root)
            .unwrap_err()
            .to_string();

        assert!(error.contains("portable graph path collision"), "{error}");
    }

    #[test]
    fn graph_discovery_rejects_nested_directories() {
        let tree = TestTree::new("nested-directory");
        let nested = tree.root.join("events/Nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("Legacy.yssbi-event"), b"{}").unwrap();

        let error = scan_graph_resource_index(&tree.root).unwrap_err();

        assert!(matches!(
            error,
            crate::project::ProjectError::InvalidProjectFormat(message)
                if message.contains("nested graph directories are not supported")
        ));
    }

    #[test]
    fn graph_discovery_rejects_external_file_redirect() {
        let tree = TestTree::new("external-file");
        let outside = tree.outside.join("External.yssbi-event");
        std::fs::write(&outside, b"{}").unwrap();
        if !try_link_file(&tree.root.join("events/External.yssbi-event"), &outside) {
            return;
        }

        assert!(scan_graph_resource_index(&tree.root).is_err());
    }

    #[test]
    fn graph_discovery_rejects_external_directory_redirect() {
        let tree = TestTree::new("external-directory");
        std::fs::write(tree.outside.join("External.yssbi-event"), b"{}").unwrap();
        link_directory(&tree.root.join("events/External"), &tree.outside);

        assert!(scan_graph_resource_index(&tree.root).is_err());
    }

    #[test]
    fn graph_discovery_rejects_directory_redirect_loop() {
        let tree = TestTree::new("directory-loop");
        link_directory(&tree.root.join("events/loop"), &tree.root.join("events"));

        assert!(scan_graph_resource_index(&tree.root).is_err());
    }
}
