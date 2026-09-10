use sqlx::{Connection, Row, SqlitePool};
use std::path::{Component, PathBuf};
use std::sync::Arc;

use crate::{DatasetFile, DatasetMetadata, DatasetPublication, DatasetStoreError, PreparedDataset};
use yss_database_contract::DatabaseId;

pub(crate) const CATALOG_VERSION: i64 = 1;
const APPLICATION_ID: i64 = 0x5953_5344;

pub(crate) async fn validate_existing(path: &std::path::Path) -> Result<(), DatasetStoreError> {
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .create_if_missing(false);
    let mut connection = sqlx::SqliteConnection::connect_with(&options).await?;
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut connection)
        .await?;
    let application: i64 = sqlx::query_scalar("PRAGMA application_id")
        .fetch_one(&mut connection)
        .await?;
    connection.close().await?;
    if version != CATALOG_VERSION || application != APPLICATION_ID {
        return Err(DatasetStoreError::UnsupportedFormat);
    }
    Ok(())
}

pub(crate) async fn initialize(pool: &SqlitePool) -> Result<(), DatasetStoreError> {
    let mut transaction = pool.begin().await?;
    sqlx::raw_sql(r#"
        CREATE TABLE datasets(id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0, head TEXT NOT NULL, next_row_id INTEGER NOT NULL);
        CREATE UNIQUE INDEX active_dataset_names ON datasets(name) WHERE deleted=0;
        CREATE TABLE generations(id TEXT PRIMARY KEY NOT NULL, dataset_id TEXT NOT NULL REFERENCES datasets(id), schema_json TEXT NOT NULL, row_count INTEGER NOT NULL);
        CREATE TABLE dataset_files(generation_id TEXT NOT NULL REFERENCES generations(id), ordinal INTEGER NOT NULL, path TEXT NOT NULL UNIQUE, row_count INTEGER NOT NULL, size_bytes INTEGER NOT NULL, PRIMARY KEY(generation_id,ordinal));
        CREATE TABLE snapshots(id TEXT PRIMARY KEY NOT NULL, dataset_id TEXT NOT NULL REFERENCES datasets(id), generation_id TEXT NOT NULL REFERENCES generations(id), schema_json TEXT NOT NULL, data_revision INTEGER NOT NULL, schema_revision INTEGER NOT NULL, row_count INTEGER NOT NULL);
        CREATE TABLE column_edits(snapshot_id TEXT NOT NULL REFERENCES snapshots(id), column_id TEXT NOT NULL, data_ipc BLOB NOT NULL, PRIMARY KEY(snapshot_id,column_id));
        CREATE TABLE inserted_rows(snapshot_id TEXT PRIMARY KEY NOT NULL REFERENCES snapshots(id), data_ipc BLOB NOT NULL);
        CREATE TABLE deleted_rows(snapshot_id TEXT NOT NULL REFERENCES snapshots(id), row_id INTEGER NOT NULL, PRIMARY KEY(snapshot_id,row_id));
        CREATE TABLE garbage_files(path TEXT PRIMARY KEY NOT NULL);
        CREATE TABLE dataset_operations(sequence INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT NOT NULL UNIQUE, dataset_id TEXT NOT NULL, before_snapshot TEXT, after_snapshot TEXT NOT NULL, published INTEGER NOT NULL DEFAULT 0);
        PRAGMA user_version=1;
        PRAGMA application_id=0x59535344;
    "#).execute(&mut *transaction).await?;
    transaction.commit().await?;
    Ok(())
}

const METADATA: &str = "SELECT d.id,d.name,d.deleted,s.id AS snapshot_id,s.generation_id,s.schema_json,s.data_revision,s.schema_revision,s.row_count FROM datasets d JOIN snapshots s ON d.head=s.id WHERE d.deleted=0";

pub(crate) async fn list(pool: &SqlitePool) -> Result<Vec<DatasetMetadata>, DatasetStoreError> {
    sqlx::query(METADATA)
        .fetch_all(pool)
        .await?
        .iter()
        .map(metadata)
        .collect()
}

pub(crate) async fn load_current(
    pool: &SqlitePool,
    database: &DatabaseId,
) -> Result<LoadedSnapshot, DatasetStoreError> {
    let mut transaction = pool.begin().await?;
    let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(METADATA);
    let row = query
        .push(" AND d.id=")
        .push_bind(database.as_str())
        .build()
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(DatasetStoreError::NotFound)?;
    let metadata = metadata(&row)?;
    let rows = sqlx::query("SELECT path,row_count,size_bytes FROM dataset_files WHERE generation_id=? ORDER BY ordinal").bind(metadata.generation_id.as_ref()).fetch_all(&mut *transaction).await?;
    let mut files = Vec::new();
    for row in rows {
        let relative_path = PathBuf::from(row.try_get::<String, _>("path")?);
        if relative_path.is_absolute()
            || relative_path
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
            || !relative_path.starts_with("datasets")
        {
            return Err(DatasetStoreError::CorruptCatalog);
        }
        files.push(DatasetFile {
            relative_path,
            row_count: usize::try_from(row.try_get::<i64, _>("row_count")?)
                .map_err(|_| DatasetStoreError::CorruptCatalog)?,
            size_bytes: u64::try_from(row.try_get::<i64, _>("size_bytes")?)
                .map_err(|_| DatasetStoreError::CorruptCatalog)?,
        });
    }
    if files.is_empty() {
        return Err(DatasetStoreError::CorruptCatalog);
    }
    let base_json: String = sqlx::query_scalar("SELECT schema_json FROM generations WHERE id=?")
        .bind(metadata.generation_id.as_ref())
        .fetch_one(&mut *transaction)
        .await?;
    let base_schema =
        Arc::new(serde_json::from_str(&base_json).map_err(|_| DatasetStoreError::CorruptCatalog)?);
    let overlay = crate::codec::load_overlay(&mut transaction, &metadata.snapshot_id).await?;
    let next_row_id = sqlx::query_scalar("SELECT next_row_id FROM datasets WHERE id=?")
        .bind(database.as_str())
        .fetch_one(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(LoadedSnapshot {
        metadata,
        files: files.into_boxed_slice(),
        base_schema,
        overlay,
        next_row_id,
    })
}

pub(crate) struct LoadedSnapshot {
    pub metadata: DatasetMetadata,
    pub files: Box<[DatasetFile]>,
    pub base_schema: arrow::datatypes::SchemaRef,
    pub overlay: yss_relational_contract::DatasetOverlay,
    pub next_row_id: i64,
}

fn metadata(row: &sqlx::sqlite::SqliteRow) -> Result<DatasetMetadata, DatasetStoreError> {
    let database_id = row.try_get::<String, _>("id")?;
    uuid::Uuid::parse_str(&database_id).map_err(|_| DatasetStoreError::CorruptCatalog)?;
    let schema =
        serde_json::from_str::<arrow::datatypes::Schema>(&row.try_get::<String, _>("schema_json")?)
            .map_err(|_| DatasetStoreError::CorruptCatalog)?;
    yss_tabular_arrow::validate_storage_schema(&schema)
        .map_err(|_| DatasetStoreError::CorruptCatalog)?;
    if yss_tabular_arrow::dataset_row_columns(&schema)
        .map_err(|_| DatasetStoreError::CorruptCatalog)?
        .is_none()
    {
        return Err(DatasetStoreError::CorruptCatalog);
    }
    Ok(DatasetMetadata {
        id: DatabaseId::from_existing(database_id.into()),
        name: row.try_get::<String, _>("name")?.into(),
        deleted: row.try_get("deleted")?,
        snapshot_id: row.try_get::<String, _>("snapshot_id")?.into(),
        generation_id: row.try_get::<String, _>("generation_id")?.into(),
        schema: Arc::new(schema),
        row_count: usize::try_from(row.try_get::<i64, _>("row_count")?)
            .map_err(|_| DatasetStoreError::CorruptCatalog)?,
        data_revision: u64::try_from(row.try_get::<i64, _>("data_revision")?)
            .map_err(|_| DatasetStoreError::CorruptCatalog)?,
        schema_revision: u64::try_from(row.try_get::<i64, _>("schema_revision")?)
            .map_err(|_| DatasetStoreError::CorruptCatalog)?,
    })
}

pub(crate) async fn commit(
    pool: &SqlitePool,
    prepared: &mut PreparedDataset,
    overlay: &crate::codec::EncodedOverlay,
) -> Result<(), DatasetStoreError> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let meta = &prepared.metadata;
    let current: Option<String> = sqlx::query_scalar("SELECT head FROM datasets WHERE id=?")
        .bind(meta.id.as_str())
        .fetch_optional(&mut *transaction)
        .await?;
    if current.as_deref() != prepared.expected_snapshot.as_deref() {
        return Err(DatasetStoreError::Conflict);
    }
    let replay: Option<i64> = sqlx::query_scalar("SELECT 1 FROM dataset_operations WHERE id=?")
        .bind(prepared.operation_id.as_ref())
        .fetch_optional(&mut *transaction)
        .await?;
    if replay.is_some() {
        return Err(DatasetStoreError::Conflict);
    }
    if current.is_none() {
        sqlx::query("INSERT INTO datasets(id,name,head,next_row_id,deleted) VALUES(?,?,?,?,?)")
            .bind(meta.id.as_str())
            .bind(meta.name.as_ref())
            .bind(meta.snapshot_id.as_ref())
            .bind(prepared.next_row_id)
            .bind(meta.deleted)
            .execute(&mut *transaction)
            .await?;
    } else {
        sqlx::query(
            "UPDATE datasets SET head=?,name=?,next_row_id=max(next_row_id,?),deleted=? WHERE id=?",
        )
        .bind(meta.snapshot_id.as_ref())
        .bind(meta.name.as_ref())
        .bind(prepared.next_row_id)
        .bind(meta.deleted)
        .bind(meta.id.as_str())
        .execute(&mut *transaction)
        .await?;
    }
    let schema = serde_json::to_string(meta.schema.as_ref())
        .map_err(|_| DatasetStoreError::InvalidSchema)?;
    if prepared.new_generation {
        sqlx::query("INSERT INTO generations(id,dataset_id,schema_json,row_count) VALUES(?,?,?,?)")
            .bind(meta.generation_id.as_ref())
            .bind(meta.id.as_str())
            .bind(&schema)
            .bind(i64::try_from(meta.row_count).map_err(|_| DatasetStoreError::InvalidSchema)?)
            .execute(&mut *transaction)
            .await?;
        for (ordinal, file) in prepared.files.iter().enumerate() {
            sqlx::query("INSERT INTO dataset_files(generation_id,ordinal,path,row_count,size_bytes) VALUES(?,?,?,?,?)").bind(meta.generation_id.as_ref()).bind(i64::try_from(ordinal).map_err(|_| DatasetStoreError::InvalidSchema)?).bind(file.relative_path.to_str().ok_or(DatasetStoreError::InvalidIdentity)?).bind(i64::try_from(file.row_count).map_err(|_| DatasetStoreError::InvalidSchema)?).bind(i64::try_from(file.size_bytes).map_err(|_| DatasetStoreError::InvalidSchema)?).execute(&mut *transaction).await?;
        }
    }
    sqlx::query("INSERT INTO snapshots(id,dataset_id,generation_id,schema_json,data_revision,schema_revision,row_count) VALUES(?,?,?,?,?,?,?)").bind(meta.snapshot_id.as_ref()).bind(meta.id.as_str()).bind(meta.generation_id.as_ref()).bind(schema).bind(i64::try_from(meta.data_revision).map_err(|_| DatasetStoreError::InvalidSchema)?).bind(i64::try_from(meta.schema_revision).map_err(|_| DatasetStoreError::InvalidSchema)?).bind(i64::try_from(meta.row_count).map_err(|_| DatasetStoreError::InvalidSchema)?).execute(&mut *transaction).await?;
    for (column_id, bytes) in &overlay.columns {
        sqlx::query("INSERT INTO column_edits(snapshot_id,column_id,data_ipc) VALUES(?,?,?)")
            .bind(meta.snapshot_id.as_ref())
            .bind(column_id.as_ref())
            .bind(bytes)
            .execute(&mut *transaction)
            .await?;
    }
    if let Some(bytes) = &overlay.inserted {
        sqlx::query("INSERT INTO inserted_rows(snapshot_id,data_ipc) VALUES(?,?)")
            .bind(meta.snapshot_id.as_ref())
            .bind(bytes)
            .execute(&mut *transaction)
            .await?;
    }
    for row_id in &overlay.deleted {
        sqlx::query("INSERT INTO deleted_rows(snapshot_id,row_id) VALUES(?,?)")
            .bind(meta.snapshot_id.as_ref())
            .bind(row_id)
            .execute(&mut *transaction)
            .await?;
    }
    sqlx::query("INSERT INTO dataset_operations(id,dataset_id,before_snapshot,after_snapshot) VALUES(?,?,?,?)").bind(prepared.operation_id.as_ref()).bind(meta.id.as_str()).bind(prepared.expected_snapshot.as_deref()).bind(meta.snapshot_id.as_ref()).execute(&mut *transaction).await?;
    // A commit I/O error can have an uncertain durable outcome. Recovery consults the catalog;
    // dropping the preparation must never remove files a committed snapshot may now reference.
    prepared.retain_files = true;
    transaction
        .commit()
        .await
        .map_err(DatasetStoreError::CommitUncertain)?;
    Ok(())
}

pub(crate) async fn pending_publications(
    pool: &SqlitePool,
) -> Result<Vec<DatasetPublication>, DatasetStoreError> {
    sqlx::query("SELECT id,dataset_id,before_snapshot,after_snapshot FROM dataset_operations WHERE published=0 ORDER BY sequence").fetch_all(pool).await?.into_iter().map(|row| Ok(DatasetPublication {
        operation_id: row.try_get::<String, _>("id")?.into(), dataset: DatabaseId::from_existing(row.try_get::<String, _>("dataset_id")?.into()), before_snapshot: row.try_get::<Option<String>, _>("before_snapshot")?.map(Into::into), after_snapshot: row.try_get::<String, _>("after_snapshot")?.into(),
    })).collect()
}

pub(crate) async fn acknowledge(
    pool: &SqlitePool,
    publication: &DatasetPublication,
) -> Result<(), DatasetStoreError> {
    let result = sqlx::query("UPDATE dataset_operations SET published=1 WHERE id=? AND dataset_id=? AND after_snapshot=? AND before_snapshot IS ?").bind(publication.operation_id.as_ref()).bind(publication.dataset.as_str()).bind(publication.after_snapshot.as_ref()).bind(publication.before_snapshot.as_deref()).execute(pool).await?;
    if result.rows_affected() != 1 {
        return Err(DatasetStoreError::Conflict);
    }
    Ok(())
}

pub(crate) async fn committed_files(pool: &SqlitePool) -> Result<Vec<PathBuf>, DatasetStoreError> {
    Ok(
        sqlx::query_scalar::<_, String>("SELECT path FROM dataset_files")
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(PathBuf::from)
            .collect(),
    )
}

pub(crate) async fn collect_garbage(
    pool: &SqlitePool,
    leased: &std::collections::BTreeSet<Box<str>>,
    orphans: &[PathBuf],
) -> Result<Vec<PathBuf>, DatasetStoreError> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    for path in orphans {
        sqlx::query("INSERT OR IGNORE INTO garbage_files(path) VALUES(?)")
            .bind(path.to_str().ok_or(DatasetStoreError::InvalidIdentity)?)
            .execute(&mut *transaction)
            .await?;
    }
    let mut protected = leased.clone();
    protected.extend(
        sqlx::query_scalar::<_, String>("SELECT head FROM datasets WHERE deleted=0")
            .fetch_all(&mut *transaction)
            .await?
            .into_iter()
            .map(Into::into),
    );
    for row in sqlx::query(
        "SELECT before_snapshot,after_snapshot FROM dataset_operations WHERE published=0",
    )
    .fetch_all(&mut *transaction)
    .await?
    {
        if let Some(before) = row.try_get::<Option<String>, _>("before_snapshot")? {
            protected.insert(before.into());
        }
        protected.insert(row.try_get::<String, _>("after_snapshot")?.into());
    }
    for id in sqlx::query_scalar::<_, String>("SELECT id FROM snapshots")
        .fetch_all(&mut *transaction)
        .await?
    {
        if protected.contains(id.as_str()) {
            continue;
        }
        sqlx::query("DELETE FROM column_edits WHERE snapshot_id=?")
            .bind(&id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM inserted_rows WHERE snapshot_id=?")
            .bind(&id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM deleted_rows WHERE snapshot_id=?")
            .bind(&id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM snapshots WHERE id=?")
            .bind(&id)
            .execute(&mut *transaction)
            .await?;
    }
    let generations = sqlx::query_scalar::<_, String>("SELECT id FROM generations WHERE NOT EXISTS(SELECT 1 FROM snapshots WHERE generation_id=generations.id)").fetch_all(&mut *transaction).await?;
    for generation in generations {
        sqlx::query("INSERT OR IGNORE INTO garbage_files(path) SELECT path FROM dataset_files WHERE generation_id=?").bind(&generation).execute(&mut *transaction).await?;
        sqlx::query("DELETE FROM dataset_files WHERE generation_id=?")
            .bind(&generation)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM generations WHERE id=?")
            .bind(&generation)
            .execute(&mut *transaction)
            .await?;
    }
    let paths = sqlx::query_scalar::<_, String>("SELECT path FROM garbage_files")
        .fetch_all(&mut *transaction)
        .await?;
    let paths = paths
        .into_iter()
        .map(|path| {
            let path = PathBuf::from(path);
            if path.is_absolute()
                || path
                    .components()
                    .any(|c| !matches!(c, Component::Normal(_)))
                || !path.starts_with("datasets")
            {
                return Err(DatasetStoreError::CorruptCatalog);
            }
            Ok(path)
        })
        .collect::<Result<Vec<_>, _>>()?;
    transaction.commit().await?;
    Ok(paths)
}

pub(crate) async fn finish_garbage(
    pool: &SqlitePool,
    path: &std::path::Path,
) -> Result<(), DatasetStoreError> {
    sqlx::query("DELETE FROM garbage_files WHERE path=?")
        .bind(path.to_str().ok_or(DatasetStoreError::InvalidIdentity)?)
        .execute(pool)
        .await?;
    Ok(())
}
