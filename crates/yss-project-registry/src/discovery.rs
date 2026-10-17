use std::path::{Path, PathBuf};

use thiserror::Error;
use walkdir::WalkDir;
use yss_filesystem::metadata_is_redirect;
use yss_project_layout::PROJECT_METADATA_FILE;
use yss_project_model::{DEFAULT_PROJECT_NAME, normalize_project_name};
use yss_project_progress::ProjectTaskCancellation;

#[derive(Debug, Error)]
pub(super) enum ProjectDiscoveryError {
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

pub(super) fn discover_project_metadata_files(
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
    if !root_metadata.is_dir() || metadata_is_redirect(&root_metadata) {
        return Err(ProjectDiscoveryError::InvalidRoot);
    }
    let mut found = Vec::new();
    let mut entries = WalkDir::new(root)
        .follow_links(false)
        .follow_root_links(false)
        .into_iter();
    while let Some(path) = next_metadata_file(&mut entries, cancellation)? {
        found.push(path);
    }
    if cancellation.is_cancelled() {
        return Err(ProjectDiscoveryError::Cancelled);
    }
    found.sort();
    found.dedup();
    Ok(found)
}

pub(super) fn project_name_from_metadata_path(metadata_path: &Path) -> String {
    metadata_path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .map(normalize_project_name)
        .unwrap_or_else(|| DEFAULT_PROJECT_NAME.into())
}

fn next_metadata_file(
    entries: &mut walkdir::IntoIter,
    cancellation: &ProjectTaskCancellation,
) -> Result<Option<PathBuf>, ProjectDiscoveryError> {
    loop {
        if cancellation.is_cancelled() {
            return Err(ProjectDiscoveryError::Cancelled);
        }
        let Some(entry) = entries.next() else {
            return Ok(None);
        };
        let entry = entry.map_err(std::io::Error::from)?;
        if !entry.file_type().is_dir() {
            continue;
        }
        #[cfg(windows)]
        if metadata_is_redirect(&entry.metadata().map_err(std::io::Error::from)?) {
            entries.skip_current_dir();
            continue;
        }
        let depth = entry.depth();
        let path = entry.into_path();
        // A user-selected root is scanned even if its name is normally skipped.
        if depth > 0 && should_skip_dir(&path) {
            entries.skip_current_dir();
            continue;
        }
        // Probe the canonical path so filename matching follows the filesystem.
        let metadata_path = path.join(PROJECT_METADATA_FILE);
        match std::fs::symlink_metadata(&metadata_path) {
            Ok(metadata) if metadata.is_file() && !metadata_is_redirect(&metadata) => {
                return Ok(Some(metadata_path));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(ProjectDiscoveryError::Io(error)),
        }
    }
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
    fn project_names_are_derived_from_the_metadata_parent() {
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
    fn discover_stops_when_cancelled() {
        let root = TestDirectory::new("cancel");
        std::fs::write(root.path().join(PROJECT_METADATA_FILE), "{}").unwrap();
        std::fs::create_dir_all(root.path().join("alpha")).unwrap();
        std::fs::write(root.path().join("alpha/metadata.yssbi"), "{}").unwrap();

        let registry = ProjectTaskCancellationRegistry::new();
        let cancellation = registry.begin();
        registry.cancel_active();
        let error = discover_project_metadata_files(root.path(), &cancellation).unwrap_err();
        assert!(matches!(error, ProjectDiscoveryError::Cancelled));

        let cancellation = registry.begin();
        let mut entries = WalkDir::new(root.path()).into_iter();
        assert_eq!(
            next_metadata_file(&mut entries, &cancellation).unwrap(),
            Some(root.path().join(PROJECT_METADATA_FILE))
        );
        registry.cancel_active();
        assert!(matches!(
            next_metadata_file(&mut entries, &cancellation),
            Err(ProjectDiscoveryError::Cancelled)
        ));
    }

    #[test]
    fn invalid_roots_and_walk_errors_do_not_become_successful_scans() {
        let temporary = TestDirectory::new("errors");
        let ordinary_file = temporary.path().join("file");
        std::fs::write(&ordinary_file, "{}").unwrap();
        let registry = ProjectTaskCancellationRegistry::new();
        let cancellation = registry.begin();
        for root in [temporary.path().join("missing"), ordinary_file] {
            assert!(matches!(
                discover_project_metadata_files(&root, &cancellation),
                Err(ProjectDiscoveryError::InvalidRoot)
            ));
        }
        assert!(matches!(
            discover_project_metadata_files(Path::new("invalid\0root"), &cancellation),
            Err(ProjectDiscoveryError::Io(_))
        ));

        // A root removed after admission must surface the traversal I/O error.
        let removed = temporary.path().join("removed");
        std::fs::create_dir(&removed).unwrap();
        let mut entries = WalkDir::new(&removed).into_iter();
        std::fs::remove_dir(&removed).unwrap();
        let error = next_metadata_file(&mut entries, &cancellation).unwrap_err();
        assert!(matches!(error, ProjectDiscoveryError::Io(error)
            if error.kind() == std::io::ErrorKind::NotFound));
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

    #[cfg(windows)]
    #[test]
    fn discovery_skips_metadata_file_reparse_points() -> std::io::Result<()> {
        use std::os::windows::fs::MetadataExt;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_REPARSE_POINT, REPARSE_GUID_DATA_BUFFER,
        };
        use windows_sys::Win32::System::IO::DeviceIoControl;
        use windows_sys::Win32::System::Ioctl::{
            FSCTL_DELETE_REPARSE_POINT, FSCTL_SET_REPARSE_POINT,
        };

        fn update_reparse_point(file: &std::fs::File, control: u32) -> std::io::Result<()> {
            // A private non-Microsoft tag needs no symlink privilege or installed filter.
            // Its empty GUID buffer works for both setting and deleting the fixture.
            let data = REPARSE_GUID_DATA_BUFFER {
                ReparseTag: 0x0000_0042,
                ReparseGuid: windows_sys::core::GUID::from_u128(
                    0x4de346d6_73e9_4d73_b074_2f27f0bdc558,
                ),
                ..Default::default()
            };
            let mut returned = 0;
            // SAFETY: the owned file handle and initialized header remain alive for
            // this synchronous call; no output buffer or overlapped I/O is used.
            let success = unsafe {
                DeviceIoControl(
                    file.as_raw_handle(),
                    control,
                    std::ptr::from_ref(&data).cast(),
                    std::mem::offset_of!(REPARSE_GUID_DATA_BUFFER, GenericReparseBuffer) as u32,
                    std::ptr::null_mut(),
                    0,
                    &mut returned,
                    std::ptr::null_mut(),
                )
            };
            if success == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        }

        let root = TestDirectory::new("metadata-reparse");
        let metadata_path = root.path().join(PROJECT_METADATA_FILE);
        let file = std::fs::File::create_new(&metadata_path)?;
        update_reparse_point(&file, FSCTL_SET_REPARSE_POINT)?;

        let registry = ProjectTaskCancellationRegistry::new();
        let cancellation = registry.begin();
        let metadata = std::fs::symlink_metadata(&metadata_path);
        let found = discover_project_metadata_files(root.path(), &cancellation);

        // Remove the tag through the original handle before assertions or recursive
        // fixture cleanup; no path is reopened through the unknown reparse point.
        update_reparse_point(&file, FSCTL_DELETE_REPARSE_POINT)?;
        drop(file);

        let metadata = metadata?;
        assert!(metadata.is_file());
        assert!(!metadata.file_type().is_symlink());
        assert_ne!(metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT, 0);
        assert!(found.unwrap().is_empty());
        assert_eq!(
            discover_project_metadata_files(root.path(), &cancellation).unwrap(),
            vec![metadata_path]
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn discovery_rejects_junction_roots_and_skips_junction_subtrees() {
        use std::os::windows::process::CommandExt;

        let root = TestDirectory::new("junction-root");
        let outside = TestDirectory::new("junction-outside");
        std::fs::write(outside.path().join(PROJECT_METADATA_FILE), "{}").unwrap();
        let redirect = root.path().join("redirect");
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&redirect)
            .arg(outside.path())
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "junction creation failed: {output:?}"
        );

        let registry = ProjectTaskCancellationRegistry::new();
        let cancellation = registry.begin();
        assert!(
            discover_project_metadata_files(root.path(), &cancellation)
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            discover_project_metadata_files(&redirect, &cancellation),
            Err(ProjectDiscoveryError::InvalidRoot)
        ));
        std::fs::remove_dir(&redirect).unwrap();
        assert!(outside.path().join(PROJECT_METADATA_FILE).is_file());
    }
}
