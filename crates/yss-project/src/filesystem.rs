//! Project-specific path interpretation and manifest admission on top of filesystem primitives.
pub use yss_project_layout::project_root_from_path;

#[cfg(test)]
mod tests {
    use super::*;
    use yss_filesystem::NormalizedRoot;
    use yss_project_layout::PROJECT_METADATA_FILE;

    #[test]
    fn project_entry_paths_resolve_to_directories() {
        let root =
            std::env::temp_dir().join(format!("project-root-policy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let metadata = root.join(PROJECT_METADATA_FILE);
        std::fs::write(&metadata, "{}").unwrap();
        let normalized = NormalizedRoot::from_path(&root).unwrap();
        assert_eq!(
            NormalizedRoot::from_path(project_root_from_path(&metadata)).unwrap(),
            normalized
        );
        assert_eq!(
            NormalizedRoot::from_path(project_root_from_path(root.join("METADATA.YSSBI"))).unwrap(),
            normalized
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
