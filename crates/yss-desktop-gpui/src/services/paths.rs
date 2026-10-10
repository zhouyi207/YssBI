use anyhow::{Context, Result};
use directories::BaseDirs;
use std::path::PathBuf;
use yss_application::runtime::ApplicationPaths;

const APPLICATION_ID: &str = "com.zjy.yssbi";

pub(super) struct NativePaths {
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
        let packaged = std::env::current_exe()?
            .parent()
            .context(crate::text::t("native.services.executableParentMissing"))?
            .join("resources/samples");
        let samples = if packaged.is_dir() {
            packaged
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/samples")
        };
        Ok(Self {
            application: ApplicationPaths {
                app_data_dir: data,
                samples_dir: samples,
            },
            logs,
        })
    }
}
