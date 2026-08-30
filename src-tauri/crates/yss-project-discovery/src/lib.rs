use std::path::{Path, PathBuf};

use thiserror::Error;
use yss_project_layout::PROJECT_METADATA_FILE;
use yss_project_progress::ProjectTaskCancellation;

pub const DEFAULT_PROJECT_NAME: &str = "\u{672a}\u{547d}\u{540d}\u{9879}\u{76ee}";

#[derive(Debug, Error)]
pub enum ProjectDiscoveryError {
    #[error("project discovery was cancelled")]
    Cancelled,
    #[error("project discovery root must be a directory")]
    InvalidRoot,
    #[error("project discovery I/O failed")]
    Io(#[from] std::io::Error),
}

const SKIP_DIR_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "$recycle.bin",
    "system volume information",
];

pub fn normalize_project_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        DEFAULT_PROJECT_NAME.into()
    } else {
        name.into()
    }
}

pub fn discover_project_metadata_files(
    root: &Path,
    cancellation: &ProjectTaskCancellation,
) -> Result<Vec<PathBuf>, ProjectDiscoveryError> {
    if cancellation.is_cancelled() {
        return Err(ProjectDiscoveryError::Cancelled);
    }
    let root_metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ProjectDiscoveryError::InvalidRoot);
        }
        Err(error) => return Err(ProjectDiscoveryError::Io(error)),
    };
    if !root_metadata.is_dir() || is_redirect(root, &root_metadata.file_type())? {
        return Err(ProjectDiscoveryError::InvalidRoot);
    }
    let mut found = Vec::new();
    walk_for_metadata(root, &mut found, cancellation)?;
    if cancellation.is_cancelled() {
        return Err(ProjectDiscoveryError::Cancelled);
    }
    found.sort();
    found.dedup();
    Ok(found)
}

pub fn project_name_from_metadata_path(metadata_path: &Path) -> String {
    metadata_path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .map(normalize_project_name)
        .unwrap_or_else(|| DEFAULT_PROJECT_NAME.into())
}

fn walk_for_metadata(
    directory: &Path,
    found: &mut Vec<PathBuf>,
    cancellation: &ProjectTaskCancellation,
) -> Result<(), ProjectDiscoveryError> {
    if cancellation.is_cancelled() {
        return Err(ProjectDiscoveryError::Cancelled);
    }

    let metadata_path = directory.join(PROJECT_METADATA_FILE);
    match std::fs::symlink_metadata(&metadata_path) {
        Ok(metadata)
            if metadata.is_file() && !is_redirect(&metadata_path, &metadata.file_type())? =>
        {
            found.push(metadata_path);
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(ProjectDiscoveryError::Io(error)),
    }

    for entry in std::fs::read_dir(directory)? {
        if cancellation.is_cancelled() {
            return Err(ProjectDiscoveryError::Cancelled);
        }
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if is_redirect(&path, &file_type)? || !file_type.is_dir() {
            continue;
        }
        if should_skip_dir(&path) {
            continue;
        }
        walk_for_metadata(&path, found, cancellation)?;
    }
    Ok(())
}

fn is_redirect(_path: &Path, file_type: &std::fs::FileType) -> Result<bool, std::io::Error> {
    if file_type.is_symlink() {
        return Ok(true);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        let metadata = std::fs::symlink_metadata(_path)?;
        Ok(metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
    }
    #[cfg(not(windows))]
    Ok(false)
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            let lower = name.to_ascii_lowercase();
            SKIP_DIR_NAMES.iter().any(|skip| lower == *skip)
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use yss_project_progress::ProjectTaskCancellationRegistry;

    static NEXT_TEST_DIRECTORY_ID: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let id = NEXT_TEST_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "yssbi-project-discovery-{label}-{}-{id}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn project_names_are_trimmed_and_blank_names_use_the_canonical_default() {
        assert_eq!(normalize_project_name("  Alpha  "), "Alpha");
        assert_eq!(normalize_project_name("  \t  "), DEFAULT_PROJECT_NAME);
        assert_eq!(
            project_name_from_metadata_path(Path::new("alpha/metadata.yssbi")),
            "alpha"
        );
        assert_eq!(
            project_name_from_metadata_path(Path::new(PROJECT_METADATA_FILE)),
            DEFAULT_PROJECT_NAME
        );
    }

    #[test]
    fn discover_nested_metadata_files_without_build_or_dependency_trees() {
        let root = TestDirectory::new("nested");
        std::fs::create_dir_all(root.path().join("alpha")).unwrap();
        std::fs::create_dir_all(root.path().join("nested/beta")).unwrap();
        std::fs::create_dir_all(root.path().join("target/ignored")).unwrap();
        std::fs::create_dir_all(root.path().join("node_modules/ignored")).unwrap();
        std::fs::write(root.path().join("alpha/metadata.yssbi"), "{}").unwrap();
        std::fs::write(root.path().join("nested/beta/metadata.yssbi"), "{}").unwrap();
        std::fs::write(root.path().join("target/ignored/metadata.yssbi"), "{}").unwrap();
        std::fs::write(
            root.path().join("node_modules/ignored/metadata.yssbi"),
            "{}",
        )
        .unwrap();

        let registry = ProjectTaskCancellationRegistry::new();
        let cancellation = registry.begin();
        let found = discover_project_metadata_files(root.path(), &cancellation).unwrap();
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|path| {
            !path.to_string_lossy().contains("target")
                && !path.to_string_lossy().contains("node_modules")
        }));
    }

    #[test]
    fn discover_stops_when_cancelled() {
        let root = TestDirectory::new("cancel");
        std::fs::create_dir_all(root.path().join("alpha")).unwrap();
        std::fs::write(root.path().join("alpha/metadata.yssbi"), "{}").unwrap();

        let registry = ProjectTaskCancellationRegistry::new();
        let cancellation = registry.begin();
        registry.cancel_active();
        let error = discover_project_metadata_files(root.path(), &cancellation).unwrap_err();
        assert!(matches!(error, ProjectDiscoveryError::Cancelled));
    }

    #[cfg(unix)]
    #[test]
    fn discovery_does_not_follow_directory_or_metadata_symlinks() {
        let root = TestDirectory::new("symlink-root");
        let outside = TestDirectory::new("symlink-outside");
        std::fs::write(outside.path().join(PROJECT_METADATA_FILE), "{}").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("redirect")).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join(PROJECT_METADATA_FILE),
            root.path().join(PROJECT_METADATA_FILE),
        )
        .unwrap();

        let registry = ProjectTaskCancellationRegistry::new();
        let cancellation = registry.begin();
        let found = discover_project_metadata_files(root.path(), &cancellation).unwrap();
        assert!(found.is_empty());

        let redirected_root = root.path().join("redirect");
        let error = discover_project_metadata_files(&redirected_root, &cancellation).unwrap_err();
        assert!(matches!(error, ProjectDiscoveryError::InvalidRoot));
    }
}
