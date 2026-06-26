use std::path::{Path, PathBuf};

/// Strip Windows extended-length prefixes (`\\?\`, `\\?\UNC\`) for display and stable storage.
pub fn format_path_for_user(path: &str) -> String {
    format_path_for_user_path(&PathBuf::from(path))
}

pub fn format_path_for_user_path(path: &Path) -> String {
    strip_windows_extended_prefix(&path.to_string_lossy())
}

fn strip_windows_extended_prefix(path: &str) -> String {
    #[cfg(windows)]
    {
        if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{rest}");
        }
        if let Some(rest) = path.strip_prefix(r"\\?\") {
            return rest.to_string();
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_extended_drive_prefix() {
        assert_eq!(
            format_path_for_user(r"\\?\C:\Users\demo\project\metadata.yssbi"),
            r"C:\Users\demo\project\metadata.yssbi"
        );
    }

    #[test]
    fn strips_extended_unc_prefix() {
        assert_eq!(
            format_path_for_user(r"\\?\UNC\server\share\project\metadata.yssbi"),
            r"\\server\share\project\metadata.yssbi"
        );
    }

    #[test]
    fn leaves_unix_paths_unchanged() {
        assert_eq!(
            format_path_for_user("/home/demo/project/metadata.yssbi"),
            "/home/demo/project/metadata.yssbi"
        );
    }
}
