use anyhow::{Context, Result};
use directories::BaseDirs;
use std::path::PathBuf;
use yss_application::runtime::ApplicationPaths;
use yss_installation_paths::samples_directory;

const APPLICATION_ID: &str = "com.zjy.yssbi";

pub(crate) struct NativePaths {
    pub application: ApplicationPaths,
    pub logs: PathBuf,
}

impl NativePaths {
    pub fn resolve() -> Result<Self> {
        let base =
            BaseDirs::new().context(crate::text::t("native.services.dataDirectoryUnavailable"))?;
        let (data, logs) = if let Some(path) = std::env::var_os("YSSBI_APP_DATA_DIR") {
            let data = PathBuf::from(path);
            anyhow::ensure!(
                data.is_absolute(),
                crate::text::t("native.services.absolutePathRequired")
            );
            let logs = data.join("logs");
            (data, logs)
        } else {
            let data = base.data_dir().join(APPLICATION_ID);
            let logs = if cfg!(target_os = "macos") {
                base.home_dir().join("Library/Logs").join(APPLICATION_ID)
            } else {
                base.data_local_dir().join(APPLICATION_ID).join("logs")
            };
            (data, logs)
        };
        let executable = std::env::current_exe()?;
        let executable_directory = executable
            .parent()
            .context(crate::text::t("native.services.executableParentMissing"))?;
        let samples = samples_directory(executable_directory);
        Ok(Self {
            application: ApplicationPaths {
                app_data_dir: data,
                samples_dir: samples,
            },
            logs,
        })
    }
}
