use std::path::{Path, PathBuf};

pub fn samples_directory(executable_directory: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    if executable_directory.ends_with("Contents/MacOS") {
        let bundled = executable_directory.join("../Resources/samples");
        if bundled.is_dir() {
            return bundled;
        }
    }
    let adjacent = executable_directory.join("resources/samples");
    if adjacent.is_dir() {
        adjacent
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/samples")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_samples_take_precedence_over_checkout() -> std::io::Result<()> {
        let installation = tempfile::tempdir()?;
        let samples = installation.path().join("resources/samples");
        std::fs::create_dir_all(&samples)?;
        assert_eq!(samples_directory(installation.path()), samples);
        Ok(())
    }

    #[test]
    fn missing_or_non_directory_installation_uses_checkout() -> std::io::Result<()> {
        let installation = tempfile::tempdir()?;
        let checkout = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/samples");
        assert_eq!(samples_directory(installation.path()), checkout);
        let resources = installation.path().join("resources");
        std::fs::create_dir(&resources)?;
        std::fs::write(resources.join("samples"), b"not a directory")?;
        assert_eq!(samples_directory(installation.path()), checkout);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn app_bundle_uses_contents_resources() -> std::io::Result<()> {
        let installation = tempfile::tempdir()?;
        let executable_directory = installation.path().join("YssBI.app/Contents/MacOS");
        let samples = installation
            .path()
            .join("YssBI.app/Contents/Resources/samples");
        std::fs::create_dir_all(&executable_directory)?;
        std::fs::create_dir_all(&samples)?;
        assert_eq!(
            samples_directory(&executable_directory).canonicalize()?,
            samples.canonicalize()?,
        );
        Ok(())
    }
}
