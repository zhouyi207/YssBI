use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{FromRow, SqlitePool};
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub const PROJECT_METADATA_FILE: &str = "metadata.yssbi";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub last_opened_at: Option<String>,
    pub is_favorite: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPathValidation {
    pub ok: bool,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyProjectRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    pub last_opened_at: String,
    #[serde(default)]
    pub is_favorite: Option<bool>,
}

#[derive(Debug, FromRow)]
struct ProjectRecordRow {
    id: String,
    name: String,
    path: String,
    created_at: String,
    last_opened_at: Option<String>,
    is_favorite: i64,
}

impl ProjectRecordRow {
    fn into_record(self) -> ProjectRecord {
        ProjectRecord {
            id: self.id,
            name: self.name,
            path: normalize_existing_path(&self.path).unwrap_or(self.path),
            created_at: self.created_at,
            last_opened_at: self.last_opened_at,
            is_favorite: self.is_favorite != 0,
        }
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
                is_favorite INTEGER NOT NULL DEFAULT 0
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
            SELECT id, name, path, created_at, last_opened_at, is_favorite
            FROM projects
            ORDER BY is_favorite DESC,
                     (last_opened_at IS NULL),
                     last_opened_at DESC,
                     name COLLATE NOCASE ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(ProjectRecordRow::into_record)
            .collect())
    }

    pub async fn register_project(&self, name: &str, path: &str) -> Result<ProjectRecord, String> {
        let name = normalize_project_name(name);
        let path = normalize_existing_path(path)?;
        let id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO projects (id, name, path, created_at, last_opened_at, is_favorite)
            VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 0)
            ON CONFLICT(path) DO UPDATE SET
                last_opened_at = excluded.last_opened_at
            "#,
        )
        .bind(&id)
        .bind(&name)
        .bind(&path)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        self.fetch_by_path(&path)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "写入项目记录后读取失败".to_string())
    }

    pub async fn migrate_legacy_projects(
        &self,
        projects: Vec<LegacyProjectRecord>,
    ) -> Result<(), String> {
        for project in projects {
            let Ok(path) = normalize_existing_path(&project.path) else {
                continue;
            };
            let id = if project.id.trim().is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                project.id
            };
            let name = normalize_project_name(&project.name);
            let favorite = if project.is_favorite.unwrap_or(false) {
                1
            } else {
                0
            };
            sqlx::query(
                r#"
                INSERT INTO projects (id, name, path, created_at, last_opened_at, is_favorite)
                VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), ?, ?)
                ON CONFLICT(path) DO UPDATE SET
                    name = excluded.name,
                    last_opened_at = COALESCE(excluded.last_opened_at, last_opened_at),
                    is_favorite = MAX(is_favorite, excluded.is_favorite)
                "#,
            )
            .bind(id)
            .bind(name)
            .bind(path)
            .bind(Some(project.last_opened_at))
            .bind(favorite)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    async fn fetch_by_path(&self, path: &str) -> Result<Option<ProjectRecord>, sqlx::Error> {
        let row = sqlx::query_as::<_, ProjectRecordRow>(
            r#"
            SELECT id, name, path, created_at, last_opened_at, is_favorite
            FROM projects
            WHERE path = ?
            "#,
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(ProjectRecordRow::into_record))
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
            SELECT id, name, path, created_at, last_opened_at, is_favorite
            FROM projects
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(row.map(ProjectRecordRow::into_record))
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

    pub async fn scan_directory(&self, directory: &str) -> Result<crate::project::ScanProjectsResult, String> {
        use crate::project::{discover_project_metadata_files, project_name_from_metadata_path, ScanProjectsResult};
        use std::path::PathBuf;

        let root = PathBuf::from(directory.trim());
        let metadata_files = discover_project_metadata_files(&root)?;
        let discovered = metadata_files.len();
        let mut newly_registered = 0;
        let mut projects = Vec::with_capacity(discovered);

        for metadata_path in metadata_files {
            let path = metadata_path.to_string_lossy().into_owned();
            let Ok(normalized) = normalize_existing_path(&path) else {
                continue;
            };
            let name = project_name_from_metadata_path(&metadata_path);
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
            Ok(docs.to_string_lossy().into_owned())
        } else {
            Ok(userprofile)
        }
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var("HOME").map_err(|_| "无法读取 HOME".to_string())?;
        let docs = PathBuf::from(&home).join("Documents");
        if docs.is_dir() {
            Ok(docs.to_string_lossy().into_owned())
        } else {
            Ok(home)
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

/// Whether two paths refer to the same YssBI project on disk.
pub fn paths_refer_to_same_project(a: &str, b: &str) -> bool {
    match (normalize_existing_path(a), normalize_existing_path(b)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
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
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| format!("无法解析项目路径: {e}"))
}
