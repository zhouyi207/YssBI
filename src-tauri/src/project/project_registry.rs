use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{FromRow, SqlitePool};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use super::{
    NormalizedProjectRoot, ProjectRootBinding, ProjectRootIdentity, format_path_for_user,
    format_path_for_user_path,
};

pub const PROJECT_METADATA_FILE: &str = "metadata.yssbi";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectRootIdentityState {
    Valid,
    Invalid,
}

impl ProjectRootIdentityState {
    fn from_stored(value: &str) -> Result<Self, sqlx::Error> {
        match value {
            "valid" => Ok(Self::Valid),
            "invalid" => Ok(Self::Invalid),
            _ => Err(sqlx::Error::Decode(
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown project root identity state '{value}'"),
                )
                .into(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub last_opened_at: Option<String>,
    pub is_favorite: bool,
    pub root_identity: ProjectRootIdentity,
    pub root_identity_state: ProjectRootIdentityState,
}

impl ProjectRecord {
    pub fn deletion_identity(&self) -> Option<&ProjectRootIdentity> {
        (self.root_identity_state == ProjectRootIdentityState::Valid
            && !self.root_identity.as_str().is_empty())
        .then_some(&self.root_identity)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupInvalidProjectsResult {
    pub removed: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPathValidation {
    pub ok: bool,
    pub message: Option<String>,
}

#[derive(Debug, FromRow)]
struct ProjectRecordRow {
    id: String,
    name: String,
    path: String,
    created_at: String,
    last_opened_at: Option<String>,
    is_favorite: i64,
    root_identity: String,
    root_identity_state: String,
}

impl ProjectRecordRow {
    fn into_record(self) -> Result<ProjectRecord, sqlx::Error> {
        Ok(ProjectRecord {
            id: self.id,
            name: self.name,
            path: normalize_existing_path(&self.path).unwrap_or(self.path),
            created_at: self.created_at,
            last_opened_at: self.last_opened_at,
            is_favorite: self.is_favorite != 0,
            root_identity: ProjectRootIdentity::from_stored(self.root_identity),
            root_identity_state: ProjectRootIdentityState::from_stored(&self.root_identity_state)?,
        })
    }
}

pub struct ProjectRegistry {
    pool: SqlitePool,
    path: PathBuf,
}

impl ProjectRegistry {
    pub async fn init(app_dir: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let db_path = app_dir.join("db").join("projects.sqlite");
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
        let options = SqliteConnectOptions::from_str(&url)?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        let registry = Self {
            pool,
            path: db_path,
        };
        registry.ensure_schema().await?;
        Ok(registry)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    async fn ensure_schema(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                last_opened_at TEXT,
                is_favorite INTEGER NOT NULL DEFAULT 0,
                root_identity TEXT NOT NULL DEFAULT '',
                root_identity_state TEXT NOT NULL DEFAULT 'invalid'
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectRecord>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ProjectRecordRow>(
            r#"
            SELECT id, name, path, created_at, last_opened_at, is_favorite, root_identity,
                   root_identity_state
            FROM projects
            ORDER BY is_favorite DESC,
                     (last_opened_at IS NULL),
                     last_opened_at DESC,
                     name COLLATE NOCASE ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(ProjectRecordRow::into_record)
            .collect()
    }

    pub async fn register_project(&self, name: &str, path: &str) -> Result<ProjectRecord, String> {
        let name = normalize_project_name(name);
        let binding = ProjectRootBinding::for_existing(path).map_err(|error| error.to_string())?;
        let root_identity = binding
            .identity()
            .cloned()
            .ok_or_else(|| "project root identity is missing".to_string())?;
        let path = normalize_existing_path(path)?;

        if let Some(existing) = self.fetch_by_path(&path).await.map_err(|e| e.to_string())? {
            if existing.deletion_identity() != Some(&root_identity) {
                return Err("registered project root identity changed".into());
            }
            sqlx::query(
                r#"
                UPDATE projects
                SET last_opened_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                WHERE id = ?
                "#,
            )
            .bind(&existing.id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
            return self
                .fetch_by_id(&existing.id)
                .await?
                .ok_or_else(|| "写入项目记录后读取失败".to_string());
        }

        let id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO projects (
                id, name, path, created_at, last_opened_at, is_favorite, root_identity,
                root_identity_state
            )
            VALUES (
                ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 0, ?, 'valid'
            )
            "#,
        )
        .bind(&id)
        .bind(&name)
        .bind(&path)
        .bind(root_identity.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        self.fetch_by_path(&path)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "写入项目记录后读取失败".to_string())
    }

    async fn fetch_by_path_exact(
        &self,
        path: &str,
    ) -> Result<Option<ProjectRecordRow>, sqlx::Error> {
        sqlx::query_as::<_, ProjectRecordRow>(
            r#"
            SELECT id, name, path, created_at, last_opened_at, is_favorite, root_identity,
                   root_identity_state
            FROM projects
            WHERE path = ?
            "#,
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await
    }

    async fn reconcile_stored_path(&self, id: &str, path: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE projects SET path = ? WHERE id = ?")
            .bind(path)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn fetch_by_path(&self, path: &str) -> Result<Option<ProjectRecord>, sqlx::Error> {
        let normalized = normalize_existing_path(path).ok();

        if let Some(ref canonical) = normalized {
            if let Some(row) = self.fetch_by_path_exact(canonical).await? {
                return Ok(Some(row.into_record()?));
            }

            let rows = sqlx::query_as::<_, ProjectRecordRow>(
                r#"
                SELECT id, name, path, created_at, last_opened_at, is_favorite, root_identity,
                       root_identity_state
                FROM projects
                "#,
            )
            .fetch_all(&self.pool)
            .await?;

            if let Some(row) = rows.into_iter().find(|row| {
                NormalizedProjectRoot::from_project_path(&row.path).ok()
                    == NormalizedProjectRoot::from_project_path(canonical).ok()
            }) {
                let _ = self.reconcile_stored_path(&row.id, canonical).await;
                return Ok(Some(row.into_record()?));
            }
        }

        self.fetch_by_path_exact(path)
            .await?
            .map(ProjectRecordRow::into_record)
            .transpose()
    }

    #[cfg(test)]
    pub(crate) async fn fail_project_remove_for_test(&self) {
        sqlx::query(
            "CREATE TEMP TRIGGER fail_project_remove BEFORE DELETE ON projects BEGIN SELECT RAISE(FAIL, 'injected registry remove failure'); END",
        )
        .execute(&self.pool)
        .await
        .unwrap();
    }

    pub async fn remove_project(&self, id: &str) -> Result<(), String> {
        let result = sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        if result.rows_affected() == 0 {
            return Err("项目不存在".into());
        }
        Ok(())
    }

    pub async fn fetch_by_id(&self, id: &str) -> Result<Option<ProjectRecord>, String> {
        let row = sqlx::query_as::<_, ProjectRecordRow>(
            r#"
            SELECT id, name, path, created_at, last_opened_at, is_favorite, root_identity,
                   root_identity_state
            FROM projects
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        row.map(ProjectRecordRow::into_record)
            .transpose()
            .map_err(|error| error.to_string())
    }

    pub async fn toggle_favorite(&self, id: &str) -> Result<bool, String> {
        let current: Option<i64> =
            sqlx::query_scalar("SELECT is_favorite FROM projects WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        let Some(current) = current else {
            return Err("项目不存在".into());
        };
        let next = if current == 0 { 1 } else { 0 };
        sqlx::query("UPDATE projects SET is_favorite = ? WHERE id = ?")
            .bind(next)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(next != 0)
    }

    /// 从注册表移除磁盘上已不存在 `metadata.yssbi` 的项目记录（不删除项目文件）。
    pub async fn cleanup_invalid_projects(
        &self,
        progress: Option<tauri::ipc::Channel<crate::project::ProjectCleanupProgressEvent>>,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<CleanupInvalidProjectsResult, String> {
        use crate::project::{
            ProjectCleanupProgressEvent, is_picker_task_cancelled, picker_task_cancelled_error,
        };

        let emit = |event: ProjectCleanupProgressEvent| {
            if let Some(channel) = progress.as_ref() {
                let _ = channel.send(event);
            }
        };

        if is_picker_task_cancelled(&cancel) {
            return Err(picker_task_cancelled_error());
        }

        let projects = self.list_projects().await.map_err(|e| e.to_string())?;
        let total = projects.len();
        let mut removed = 0usize;

        for (index, project) in projects.iter().enumerate() {
            if is_picker_task_cancelled(&cancel) {
                return Err(picker_task_cancelled_error());
            }

            emit(ProjectCleanupProgressEvent::Checking {
                current: index + 1,
                total,
            });

            let current_identity = (project.root_identity_state == ProjectRootIdentityState::Valid)
                .then(|| ProjectRootBinding::for_existing(&project.path).ok())
                .flatten()
                .and_then(|binding| binding.identity().cloned());
            if project.root_identity_state == ProjectRootIdentityState::Valid
                && current_identity.as_ref() == Some(&project.root_identity)
            {
                continue;
            }

            self.remove_project(&project.id).await?;
            removed += 1;
            emit(ProjectCleanupProgressEvent::Removing { removed, total });
        }

        Ok(CleanupInvalidProjectsResult { removed })
    }

    pub async fn scan_directory(
        &self,
        directory: &str,
        progress: Option<tauri::ipc::Channel<crate::project::ProjectScanProgressEvent>>,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<crate::project::ScanProjectsResult, String> {
        use crate::project::{
            ProjectScanProgressEvent, ScanProjectsResult, discover_project_metadata_files,
            is_picker_task_cancelled, picker_task_cancelled_error, project_name_from_metadata_path,
        };
        use std::path::PathBuf;

        let emit = |event: ProjectScanProgressEvent| {
            if let Some(channel) = progress.as_ref() {
                let _ = channel.send(event);
            }
        };

        if is_picker_task_cancelled(&cancel) {
            return Err(picker_task_cancelled_error());
        }

        emit(ProjectScanProgressEvent::Scanning);

        let root = PathBuf::from(directory.trim());
        let metadata_files = discover_project_metadata_files(&root, &cancel)?;
        let discovered = metadata_files.len();
        emit(ProjectScanProgressEvent::Discovered { count: discovered });

        let mut newly_registered = 0;
        let mut projects = Vec::with_capacity(discovered);

        for (index, metadata_path) in metadata_files.iter().enumerate() {
            if is_picker_task_cancelled(&cancel) {
                return Err(picker_task_cancelled_error());
            }

            emit(ProjectScanProgressEvent::Registering {
                current: index + 1,
                total: discovered,
            });
            let path = metadata_path.to_string_lossy().into_owned();
            let Ok(normalized) = normalize_existing_path(&path) else {
                continue;
            };
            let name = project_name_from_metadata_path(metadata_path);
            let (record, is_new) = self.register_discovered_project(&name, &normalized).await?;
            if is_new {
                newly_registered += 1;
            }
            projects.push(record);
        }

        Ok(ScanProjectsResult {
            discovered,
            newly_registered,
            projects,
        })
    }

    async fn register_discovered_project(
        &self,
        name: &str,
        path: &str,
    ) -> Result<(ProjectRecord, bool), String> {
        if let Some(existing) = self.fetch_by_path(path).await.map_err(|e| e.to_string())? {
            return Ok((existing, false));
        }
        let record = self.register_project(name, path).await?;
        Ok((record, true))
    }
}

pub fn default_project_parent_directory() -> Result<String, String> {
    #[cfg(windows)]
    {
        let userprofile =
            std::env::var("USERPROFILE").map_err(|_| "无法读取用户目录".to_string())?;
        let docs = PathBuf::from(&userprofile).join("Documents");
        if docs.is_dir() {
            Ok(format_path_for_user_path(&docs))
        } else {
            Ok(format_path_for_user(&userprofile))
        }
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var("HOME").map_err(|_| "无法读取 HOME".to_string())?;
        let docs = PathBuf::from(&home).join("Documents");
        if docs.is_dir() {
            Ok(format_path_for_user_path(&docs))
        } else {
            Ok(format_path_for_user(&home))
        }
    }
}

pub fn validate_new_project_path(path: &str) -> ProjectPathValidation {
    let path = path.trim();
    if path.is_empty() {
        return invalid("路径不能为空");
    }

    let pb = PathBuf::from(path);
    if pb.exists() {
        if pb.is_file() {
            return invalid("项目路径必须是文件夹");
        }
        let metadata_path = pb.join(PROJECT_METADATA_FILE);
        if metadata_path.exists() {
            return invalid("该项目文件夹已包含 metadata.yssbi，请更换名称或路径");
        }
        if directory_has_entries(&pb) {
            return invalid("项目文件夹必须为空或不存在");
        }
        return ProjectPathValidation {
            ok: true,
            message: None,
        };
    }
    let Some(parent) = pb.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return invalid("无效的父路径");
    };
    if !parent.exists() || !parent.is_dir() {
        return invalid("父目录不存在或不是文件夹");
    }

    ProjectPathValidation {
        ok: true,
        message: None,
    }
}

fn directory_has_entries(path: &Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(true)
}

fn invalid(message: &str) -> ProjectPathValidation {
    ProjectPathValidation {
        ok: false,
        message: Some(message.into()),
    }
}

pub fn normalize_project_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        "未命名项目".into()
    } else {
        name.into()
    }
}

pub fn is_registered_project_valid(path: &str) -> bool {
    normalize_existing_path(path).is_ok()
}

pub fn normalize_existing_path(path: &str) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("路径不能为空".into());
    }
    let input = PathBuf::from(path);
    let pb = if input.is_dir() {
        input.join(PROJECT_METADATA_FILE)
    } else {
        input
    };
    if !pb.exists() {
        return Err("项目文件不存在".into());
    }
    if !pb.is_file() {
        return Err("项目路径必须是文件".into());
    }
    let file_name = pb
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "无效的项目文件路径".to_string())?;
    if !file_name.eq_ignore_ascii_case(PROJECT_METADATA_FILE) {
        return Err(format!("项目文件必须是 {PROJECT_METADATA_FILE}"));
    }
    std::fs::canonicalize(&pb)
        .map(|p| format_path_for_user_path(&p))
        .map_err(|e| format!("无法解析项目路径: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ProjectRootBinding;
    use std::fs;

    #[test]
    fn registry_defaults_new_rows_to_invalid_and_rejects_unknown_identity_state() {
        tauri::async_runtime::block_on(async {
            let app_dir = std::env::temp_dir().join(format!(
                "yssbi-registry-identity-state-{}",
                uuid::Uuid::new_v4()
            ));
            let registry = ProjectRegistry::init(app_dir.clone()).await.unwrap();
            sqlx::query(
                "INSERT INTO projects (id, name, path, created_at) VALUES ('state-test', 'State Test', 'missing', 'now')",
            )
            .execute(&registry.pool)
            .await
            .unwrap();

            let record = registry.fetch_by_id("state-test").await.unwrap().unwrap();
            assert_eq!(
                record.root_identity_state,
                ProjectRootIdentityState::Invalid
            );

            sqlx::query(
                "UPDATE projects SET root_identity_state = 'unmigrated' WHERE id = 'state-test'",
            )
            .execute(&registry.pool)
            .await
            .unwrap();
            let error = registry.fetch_by_id("state-test").await.unwrap_err();
            assert!(error.contains("unknown project root identity state 'unmigrated'"));

            drop(registry);
            let _ = fs::remove_dir_all(app_dir);
        });
    }

    #[test]
    fn cleanup_removes_terminal_and_replaced_valid_rows_before_reregistration() {
        tauri::async_runtime::block_on(async {
            let app_dir = std::env::temp_dir().join(format!(
                "yssbi-registry-cleanup-identity-{}",
                uuid::Uuid::new_v4()
            ));
            let terminal_root = app_dir.join("terminal");
            let valid_root = app_dir.join("valid");
            fs::create_dir_all(&terminal_root).unwrap();
            fs::create_dir_all(&valid_root).unwrap();
            let terminal_metadata = terminal_root.join(PROJECT_METADATA_FILE);
            let valid_metadata = valid_root.join(PROJECT_METADATA_FILE);
            fs::write(&terminal_metadata, "{}").unwrap();
            fs::write(&valid_metadata, "{}").unwrap();

            let registry = ProjectRegistry::init(app_dir.clone()).await.unwrap();
            let terminal = registry
                .register_project("Terminal", terminal_metadata.to_string_lossy().as_ref())
                .await
                .unwrap();
            let valid = registry
                .register_project("Valid", valid_metadata.to_string_lossy().as_ref())
                .await
                .unwrap();

            fs::remove_dir_all(&terminal_root).unwrap();
            fs::rename(&valid_root, app_dir.join("valid-original")).unwrap();
            fs::create_dir_all(&valid_root).unwrap();
            fs::write(&valid_metadata, "{}").unwrap();

            let cleanup = registry
                .cleanup_invalid_projects(
                    None,
                    std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                )
                .await
                .unwrap();

            assert_eq!(cleanup.removed, 2);
            assert!(registry.fetch_by_id(&terminal.id).await.unwrap().is_none());
            assert!(registry.fetch_by_id(&valid.id).await.unwrap().is_none());
            fs::create_dir_all(&terminal_root).unwrap();
            fs::write(&terminal_metadata, "{}").unwrap();
            let terminal_registered = registry
                .register_project(
                    "Terminal Replacement",
                    terminal_metadata.to_string_lossy().as_ref(),
                )
                .await
                .unwrap();
            let valid_registered = registry
                .register_project(
                    "Valid Replacement",
                    valid_metadata.to_string_lossy().as_ref(),
                )
                .await
                .unwrap();
            assert_eq!(
                terminal_registered.root_identity_state,
                ProjectRootIdentityState::Valid
            );
            assert_eq!(
                valid_registered.root_identity_state,
                ProjectRootIdentityState::Valid
            );

            drop(registry);
            let _ = fs::remove_dir_all(app_dir);
        });
    }

    #[test]
    fn registry_persists_native_root_identity_and_rejects_same_path_replacement() {
        tauri::async_runtime::block_on(async {
            let app_dir = std::env::temp_dir().join(format!(
                "yssbi-registry-root-identity-{}",
                uuid::Uuid::new_v4()
            ));
            let project_root = app_dir.join("project");
            fs::create_dir_all(&project_root).unwrap();
            let metadata = project_root.join(PROJECT_METADATA_FILE);
            fs::write(&metadata, "{}").unwrap();
            let registry = ProjectRegistry::init(app_dir.clone()).await.unwrap();
            let expected = ProjectRootBinding::for_existing(&metadata)
                .unwrap()
                .identity()
                .unwrap()
                .clone();

            let registered = registry
                .register_project("Original", &metadata.to_string_lossy())
                .await
                .unwrap();
            assert_eq!(registered.root_identity, expected);
            assert_eq!(
                registry
                    .fetch_by_id(&registered.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .root_identity,
                expected
            );

            fs::remove_dir_all(&project_root).unwrap();
            fs::create_dir_all(&project_root).unwrap();
            fs::write(&metadata, "{}").unwrap();
            let replacement = ProjectRootBinding::for_existing(&metadata)
                .unwrap()
                .identity()
                .unwrap()
                .clone();
            assert_ne!(replacement, expected);

            let error = registry
                .register_project("Replacement", &metadata.to_string_lossy())
                .await
                .unwrap_err();
            assert!(error.contains("identity"));
            assert_eq!(
                registry
                    .fetch_by_id(&registered.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .root_identity,
                expected
            );
            drop(registry);
            let _ = fs::remove_dir_all(app_dir);
        });
    }

    #[test]
    fn registered_project_valid_when_metadata_exists() {
        let root =
            std::env::temp_dir().join(format!("yssbi-cleanup-test-{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let metadata = root.join(PROJECT_METADATA_FILE);
        fs::write(&metadata, "{}").unwrap();

        assert!(is_registered_project_valid(&metadata.to_string_lossy()));
        assert!(!is_registered_project_valid(
            &root.join("missing-metadata.yssbi").to_string_lossy()
        ));

        let _ = fs::remove_dir_all(&root);
    }
}
