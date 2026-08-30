//! Dependency-free path presentation helpers.
//!
//! Windows extended-length prefixes are an operating-system implementation
//! detail. User-facing paths and stable stored path strings must not expose
//! them, even when normalization runs on a non-Windows host.

use std::path::Path;

/// Removes a Windows extended-length prefix from a path string.
pub fn format_path_for_user(path: &str) -> String {
    strip_windows_extended_prefix(path)
}

/// Removes a Windows extended-length prefix from a platform path.
pub fn format_path_for_user_path(path: &Path) -> String {
    strip_windows_extended_prefix(&path.to_string_lossy())
}

fn strip_windows_extended_prefix(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = path.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_extended_drive_prefix_on_every_host() {
        assert_eq!(
            format_path_for_user(r"\\?\C:\Users\demo\project\metadata.yssbi"),
            r"C:\Users\demo\project\metadata.yssbi"
        );
    }

    #[test]
    fn strips_extended_unc_prefix_on_every_host() {
        assert_eq!(
            format_path_for_user(r"\\?\UNC\server\share\project\metadata.yssbi"),
            r"\\server\share\project\metadata.yssbi"
        );
    }

    #[test]
    fn path_and_string_entry_points_share_one_semantics() {
        let path = r"\\?\C:\Users\demo\project\metadata.yssbi";
        assert_eq!(
            format_path_for_user_path(Path::new(path)),
            format_path_for_user(path)
        );
    }

    #[test]
    fn leaves_unprefixed_paths_unchanged() {
        assert_eq!(
            format_path_for_user("/home/demo/project/metadata.yssbi"),
            "/home/demo/project/metadata.yssbi"
        );
    }
}
