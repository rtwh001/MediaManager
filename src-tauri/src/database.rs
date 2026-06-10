use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, ErrorCode, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::parser::{normalized_group_key, parse_media_name};

const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_initial.sql");
const MEDIA_DETAILS_MIGRATION: &str = include_str!("../migrations/0002_media_details.sql");
const MEDIA_GROUPING_MIGRATION: &str = include_str!("../migrations/0003_media_grouping.sql");
const APP_SETTINGS_MIGRATION: &str = include_str!("../migrations/0004_app_settings.sql");
const MEDIA_BLACKLIST_MIGRATION: &str = include_str!("../migrations/0005_media_blacklist.sql");

pub struct DatabaseState {
    connection: Mutex<Connection>,
    path: PathBuf,
}

impl DatabaseState {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStatus {
    path: String,
    schema_version: i64,
    media_count: i64,
    scan_source_count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSource {
    id: i64,
    path: String,
    display_name: String,
    enabled: bool,
    recursive: bool,
    last_scanned_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaCard {
    id: i64,
    title: String,
    year: Option<i32>,
    media_type: String,
    recognition_status: String,
    file_count: i64,
    extension: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    is_missing: bool,
    file_path: Option<String>,
    file_name: Option<String>,
    file_size: Option<i64>,
    modified_at: Option<String>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    container_format: Option<String>,
    created_at: String,
    overview: Option<String>,
    user_notes: Option<String>,
    watched: bool,
    season_number: Option<i32>,
    episode_number: Option<i32>,
    duration_seconds: Option<f64>,
    hdr_format: Option<String>,
    poster_path: Option<String>,
    tag_ids: Vec<i64>,
    tag_names: Vec<String>,
    collection_ids: Vec<i64>,
    collection_names: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddScanSourceRequest {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMediaRequest {
    id: i64,
    title: String,
    year: Option<i32>,
    media_type: String,
    overview: Option<String>,
    user_notes: Option<String>,
    watched: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    id: i64,
    name: String,
    color: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTagRequest {
    name: String,
    color: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    id: i64,
    name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionRequest {
    name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMediaRelationsRequest {
    media_id: i64,
    ids: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetWatchedRequest {
    media_id: i64,
    watched: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMediaTypeRequest {
    media_id: i64,
    media_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMediaRequest {
    media_ids: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeMediaRequest {
    keeper_id: i64,
    media_ids: Vec<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryMutationResult {
    items_removed: u64,
    files_relinked: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlacklistItem {
    id: i64,
    path: String,
    file_name: String,
    media_title: Option<String>,
    scan_source_path: Option<String>,
    deleted_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreBlacklistRequest {
    ids: Vec<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanHistoryItem {
    id: i64,
    source_name: Option<String>,
    started_at: String,
    finished_at: Option<String>,
    status: String,
    files_found: i64,
    files_added: i64,
    files_updated: i64,
    files_missing: i64,
    files_ignored: i64,
    error_message: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeDuplicatesResult {
    groups_merged: u64,
    items_removed: u64,
    files_relinked: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBackupRequest {
    destination: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    path: String,
    size_bytes: u64,
    artwork_files: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreBackupRequest {
    source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreBackupResult {
    automatic_backup_path: String,
    artwork_files: u64,
    schema_version: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrateMediaPathsRequest {
    old_root: String,
    new_root: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrateMediaPathsResult {
    scan_sources_updated: u64,
    media_files_updated: u64,
    blacklist_paths_updated: u64,
}

type MediaGroupKey = (String, String, Option<i32>);
type MediaGroupMember = (i64, String);

pub fn initialize(app: &AppHandle) -> Result<DatabaseState, String> {
    let data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| format!("无法确定应用数据目录: {error}"))?;

    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("无法创建应用数据目录 {}: {error}", data_dir.display()))?;

    let database_path = data_dir.join("library.db");
    let connection = open_database(&database_path)?;

    Ok(DatabaseState {
        connection: Mutex::new(connection),
        path: database_path,
    })
}

fn open_database(path: &Path) -> Result<Connection, String> {
    let mut connection = Connection::open(path)
        .map_err(|error| format!("无法打开数据库 {}: {error}", path.display()))?;

    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(|error| format!("无法配置 SQLite: {error}"))?;

    connection
        .execute_batch(INITIAL_MIGRATION)
        .map_err(|error| format!("无法执行初始数据库迁移: {error}"))?;

    apply_migration(&mut connection, 2, MEDIA_DETAILS_MIGRATION)?;
    apply_migration(&mut connection, 3, MEDIA_GROUPING_MIGRATION)?;
    apply_migration(&mut connection, 4, APP_SETTINGS_MIGRATION)?;
    apply_migration(&mut connection, 5, MEDIA_BLACKLIST_MIGRATION)?;

    Ok(connection)
}

fn apply_migration(connection: &mut Connection, version: i64, sql: &str) -> Result<(), String> {
    let applied = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            [version],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| format!("无法检查数据库迁移 {version}: {error}"))?;

    if applied {
        return Ok(());
    }

    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始数据库迁移 {version}: {error}"))?;
    transaction
        .execute_batch(sql)
        .map_err(|error| format!("无法执行数据库迁移 {version}: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("无法提交数据库迁移 {version}: {error}"))
}

#[tauri::command]
pub fn database_status(state: State<'_, DatabaseState>) -> Result<DatabaseStatus, String> {
    let connection = lock_connection(&state)?;

    let schema_version = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("无法读取数据库版本: {error}"))?;

    let media_count = count_rows(&connection, "media_items")?;
    let scan_source_count = count_rows(&connection, "scan_sources")?;

    Ok(DatabaseStatus {
        path: state.path.to_string_lossy().into_owned(),
        schema_version,
        media_count,
        scan_source_count,
    })
}

#[tauri::command]
pub fn export_library_backup(
    app: AppHandle,
    request: ExportBackupRequest,
    state: State<'_, DatabaseState>,
) -> Result<BackupResult, String> {
    let destination = PathBuf::from(request.destination.trim());
    if destination.as_os_str().is_empty() {
        return Err("备份保存路径不能为空".to_string());
    }
    let connection = lock_connection(&state)?;
    create_backup_file(
        &connection,
        &app.package_info().version.to_string(),
        &destination,
    )
}

#[tauri::command]
pub fn restore_library_backup(
    app: AppHandle,
    request: RestoreBackupRequest,
    state: State<'_, DatabaseState>,
    scanner: State<'_, crate::scanner::ScannerState>,
) -> Result<RestoreBackupResult, String> {
    if scanner.is_running() {
        return Err("扫描进行中，无法恢复资料库".to_string());
    }
    let source = PathBuf::from(request.source.trim());
    if !source.is_file() {
        return Err("选择的备份文件不存在".to_string());
    }
    let data_dir = state
        .path
        .parent()
        .ok_or_else(|| "无法确定应用数据目录".to_string())?
        .to_path_buf();
    let restore_path = data_dir.join(format!(
        "restore-{}-{}.db",
        std::process::id(),
        unix_timestamp()
    ));
    let restore_artwork_dir = data_dir.join(format!(
        "restore-artwork-{}-{}",
        std::process::id(),
        unix_timestamp()
    ));
    fs::copy(&source, &restore_path).map_err(|error| format!("无法复制待恢复备份: {error}"))?;

    let restore_result = (|| {
        let mut restored = Connection::open(&restore_path)
            .map_err(|error| format!("无法打开备份文件: {error}"))?;
        let format_version = backup_manifest_value(&restored, "format_version")?;
        if format_version != "1" {
            return Err(format!("不支持的备份格式版本: {format_version}"));
        }
        let schema_version = backup_manifest_value(&restored, "schema_version")?
            .parse::<i64>()
            .map_err(|_| "备份中的数据库版本无效".to_string())?;
        if schema_version > 5 {
            return Err(format!(
                "此备份来自更新版本的 MediaManager（Schema v{schema_version}）"
            ));
        }
        let integrity: String = restored
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|error| format!("无法检查备份完整性: {error}"))?;
        if integrity != "ok" {
            return Err(format!("备份完整性检查失败: {integrity}"));
        }

        let backup_dir = data_dir.join("backups");
        fs::create_dir_all(&backup_dir)
            .map_err(|error| format!("无法创建自动备份目录: {error}"))?;
        let automatic_backup = backup_dir.join(format!("pre-restore-{}.mmbak", unix_timestamp()));
        let current_tmdb_token = {
            let connection = lock_connection(&state)?;
            let token = connection
                .query_row(
                    "SELECT value FROM app_settings
                     WHERE key = 'tmdb_read_access_token'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|error| format!("无法读取本机 TMDB Token: {error}"))?;
            create_backup_file(
                &connection,
                &app.package_info().version.to_string(),
                &automatic_backup,
            )?;
            token
        };

        let poster_dir = data_dir.join("cache").join("posters");
        fs::create_dir_all(&poster_dir)
            .map_err(|error| format!("无法创建海报恢复目录: {error}"))?;
        fs::create_dir_all(&restore_artwork_dir)
            .map_err(|error| format!("无法创建海报恢复暂存目录: {error}"))?;
        let artwork_files =
            restore_embedded_artwork(&mut restored, &restore_artwork_dir, &poster_dir)?;
        restored
            .execute_batch(
                "DELETE FROM app_settings WHERE key = 'tmdb_read_access_token';
                 DROP TABLE IF EXISTS backup_artwork_files;
                 DROP TABLE IF EXISTS backup_manifest;
                 PRAGMA wal_checkpoint(TRUNCATE);",
            )
            .map_err(|error| format!("无法整理恢复数据库: {error}"))?;
        drop(restored);

        let mut connection = lock_connection(&state)?;
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|error| format!("无法结束当前数据库写入: {error}"))?;
        let placeholder =
            Connection::open_in_memory().map_err(|error| format!("无法准备数据库恢复: {error}"))?;
        let old_connection = std::mem::replace(&mut *connection, placeholder);
        drop(old_connection);

        remove_sqlite_files(&state.path)?;
        if let Err(error) = fs::rename(&restore_path, &state.path) {
            fs::copy(&restore_path, &state.path)
                .map_err(|copy_error| format!("无法替换资料库: {error}; {copy_error}"))?;
        }
        match open_database(&state.path) {
            Ok(restored_connection) => {
                restore_local_tmdb_token(&restored_connection, current_tmdb_token.as_deref())?;
                *connection = restored_connection;
                copy_staged_artwork(&restore_artwork_dir, &poster_dir)?;
            }
            Err(error) => {
                let rollback_copy = copy_backup_database(&automatic_backup, &state.path);
                let rollback_connection = open_database(&state.path)
                    .map_err(|rollback_error| format!(
                        "恢复后无法打开数据库: {error}; 自动回滚失败: {rollback_copy:?}; {rollback_error}"
                    ))?;
                restore_local_tmdb_token(&rollback_connection, current_tmdb_token.as_deref())?;
                *connection = rollback_connection;
                return Err(format!("恢复失败，已回滚到恢复前资料: {error}"));
            }
        }

        Ok(RestoreBackupResult {
            automatic_backup_path: automatic_backup.to_string_lossy().into_owned(),
            artwork_files,
            schema_version,
        })
    })();

    let _ = fs::remove_file(&restore_path);
    let _ = fs::remove_dir_all(&restore_artwork_dir);
    restore_result
}

#[tauri::command]
pub fn migrate_media_paths(
    request: MigrateMediaPathsRequest,
    state: State<'_, DatabaseState>,
    scanner: State<'_, crate::scanner::ScannerState>,
) -> Result<MigrateMediaPathsResult, String> {
    if scanner.is_running() {
        return Err("扫描进行中，无法迁移媒体路径".to_string());
    }
    let old_root = normalize_root(&request.old_root)?;
    let new_root = normalize_root(&request.new_root)?;
    if old_root.eq_ignore_ascii_case(&new_root) {
        return Err("新旧媒体路径相同".to_string());
    }

    let mut connection = lock_connection(&state)?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始路径迁移: {error}"))?;
    let scan_sources_updated =
        migrate_table_paths(&transaction, "scan_sources", "path", &old_root, &new_root)?;
    let media_files_updated =
        migrate_table_paths(&transaction, "media_files", "path", &old_root, &new_root)?;
    let blacklist_paths_updated = migrate_table_paths(
        &transaction,
        "media_blacklist",
        "path",
        &old_root,
        &new_root,
    )? + migrate_table_paths(
        &transaction,
        "media_blacklist",
        "scan_source_path",
        &old_root,
        &new_root,
    )?;
    transaction
        .commit()
        .map_err(|error| format!("无法提交路径迁移: {error}"))?;

    Ok(MigrateMediaPathsResult {
        scan_sources_updated,
        media_files_updated,
        blacklist_paths_updated,
    })
}

#[tauri::command]
pub fn list_scan_sources(state: State<'_, DatabaseState>) -> Result<Vec<ScanSource>, String> {
    let connection = lock_connection(&state)?;
    let mut statement = connection
        .prepare(
            "SELECT id, path, COALESCE(display_name, path), enabled, recursive, last_scanned_at
             FROM scan_sources
             ORDER BY created_at ASC, id ASC",
        )
        .map_err(|error| format!("无法准备目录查询: {error}"))?;

    let sources = statement
        .query_map([], |row| {
            Ok(ScanSource {
                id: row.get(0)?,
                path: row.get(1)?,
                display_name: row.get(2)?,
                enabled: row.get(3)?,
                recursive: row.get(4)?,
                last_scanned_at: row.get(5)?,
            })
        })
        .map_err(|error| format!("无法查询媒体目录: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取媒体目录: {error}"))?;

    Ok(sources)
}

#[tauri::command]
pub fn add_scan_source(
    request: AddScanSourceRequest,
    state: State<'_, DatabaseState>,
) -> Result<ScanSource, String> {
    let raw_path = request.path.trim();
    if raw_path.is_empty() {
        return Err("媒体目录路径不能为空".to_string());
    }

    let path = PathBuf::from(raw_path);
    if !path.exists() {
        return Err("选择的目录不存在".to_string());
    }
    if !path.is_dir() {
        return Err("选择的路径不是目录".to_string());
    }

    let canonical_path = path
        .canonicalize()
        .map_err(|error| format!("无法读取所选目录: {error}"))?;
    let path_text = canonical_path.to_string_lossy().into_owned();
    let display_name = canonical_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(&path_text)
        .to_string();

    let connection = lock_connection(&state)?;
    if let Err(error) = connection.execute(
        "INSERT INTO scan_sources (path, display_name) VALUES (?1, ?2)",
        params![path_text, display_name],
    ) {
        if error.sqlite_error_code() == Some(ErrorCode::ConstraintViolation) {
            return Err("这个媒体目录已经添加过了".to_string());
        }
        return Err(format!("无法保存媒体目录: {error}"));
    }

    Ok(ScanSource {
        id: connection.last_insert_rowid(),
        path: path_text,
        display_name,
        enabled: true,
        recursive: true,
        last_scanned_at: None,
    })
}

#[tauri::command]
pub fn remove_scan_source(id: i64, state: State<'_, DatabaseState>) -> Result<(), String> {
    let mut connection = lock_connection(&state)?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始移除媒体目录: {error}"))?;
    let affected = transaction
        .execute("DELETE FROM scan_sources WHERE id = ?1", [id])
        .map_err(|error| format!("无法移除媒体目录: {error}"))?;

    if affected == 0 {
        return Err("要移除的媒体目录不存在".to_string());
    }

    transaction
        .execute(
            "DELETE FROM media_items
             WHERE NOT EXISTS (
                 SELECT 1 FROM media_files WHERE media_files.media_item_id = media_items.id
             )",
            [],
        )
        .map_err(|error| format!("无法清理孤立影视条目: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("无法提交目录移除: {error}"))
}

#[tauri::command]
pub fn list_media_items(state: State<'_, DatabaseState>) -> Result<Vec<MediaCard>, String> {
    let connection = lock_connection(&state)?;
    let mut statement = connection
        .prepare(
            "SELECT
                item.id,
                item.title,
                item.year,
                item.media_type,
                item.recognition_status,
                COUNT(file.id) AS file_count,
                MIN(file.extension) AS extension,
                MAX(file.width) AS width,
                MAX(file.height) AS height,
                COALESCE(MIN(file.is_missing), 0) AS is_missing,
                MIN(file.path) AS file_path,
                MIN(file.file_name) AS file_name,
                SUM(file.file_size) AS file_size,
                MAX(file.modified_at) AS modified_at,
                MAX(file.video_codec) AS video_codec,
                MAX(file.audio_codec) AS audio_codec,
                MAX(file.container_format) AS container_format,
                item.created_at,
                item.overview,
                item.user_notes,
                item.watched,
                item.season_number,
                item.episode_number,
                MAX(file.duration_seconds) AS duration_seconds,
                MAX(file.hdr_format) AS hdr_format,
                (
                    SELECT art.local_path FROM artwork art
                    WHERE art.media_item_id = item.id
                      AND art.artwork_type = 'poster'
                      AND art.is_primary = 1
                    ORDER BY art.id DESC LIMIT 1
                ) AS poster_path,
                (
                    SELECT GROUP_CONCAT(tag.id, '|')
                    FROM tags tag
                    JOIN media_tags mt ON mt.tag_id = tag.id
                    WHERE mt.media_item_id = item.id
                ) AS tag_ids,
                (
                    SELECT GROUP_CONCAT(tag.name, '|')
                    FROM tags tag
                    JOIN media_tags mt ON mt.tag_id = tag.id
                    WHERE mt.media_item_id = item.id
                ) AS tag_names,
                (
                    SELECT GROUP_CONCAT(collection.id, '|')
                    FROM collections collection
                    JOIN collection_items ci ON ci.collection_id = collection.id
                    WHERE ci.media_item_id = item.id
                ) AS collection_ids,
                (
                    SELECT GROUP_CONCAT(collection.name, '|')
                    FROM collections collection
                    JOIN collection_items ci ON ci.collection_id = collection.id
                    WHERE ci.media_item_id = item.id
                ) AS collection_names
             FROM media_items item
             LEFT JOIN media_files file ON file.media_item_id = item.id
             GROUP BY item.id
             ORDER BY item.created_at DESC, item.id DESC",
        )
        .map_err(|error| format!("无法准备影视条目查询: {error}"))?;

    let items = statement
        .query_map([], |row| {
            let tag_ids: Option<String> = row.get(26)?;
            let tag_names: Option<String> = row.get(27)?;
            let collection_ids: Option<String> = row.get(28)?;
            let collection_names: Option<String> = row.get(29)?;
            Ok(MediaCard {
                id: row.get(0)?,
                title: row.get(1)?,
                year: row.get(2)?,
                media_type: row.get(3)?,
                recognition_status: row.get(4)?,
                file_count: row.get(5)?,
                extension: row.get(6)?,
                width: row.get(7)?,
                height: row.get(8)?,
                is_missing: row.get(9)?,
                file_path: row.get(10)?,
                file_name: row.get(11)?,
                file_size: row.get(12)?,
                modified_at: row.get(13)?,
                video_codec: row.get(14)?,
                audio_codec: row.get(15)?,
                container_format: row.get(16)?,
                created_at: row.get(17)?,
                overview: row.get(18)?,
                user_notes: row.get(19)?,
                watched: row.get(20)?,
                season_number: row.get(21)?,
                episode_number: row.get(22)?,
                duration_seconds: row.get(23)?,
                hdr_format: row.get(24)?,
                poster_path: row.get(25)?,
                tag_ids: split_i64_list(tag_ids),
                tag_names: split_string_list(tag_names),
                collection_ids: split_i64_list(collection_ids),
                collection_names: split_string_list(collection_names),
            })
        })
        .map_err(|error| format!("无法查询影视条目: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取影视条目: {error}"))?;

    Ok(items)
}

#[tauri::command]
pub fn update_media_item(
    request: UpdateMediaRequest,
    state: State<'_, DatabaseState>,
) -> Result<(), String> {
    let title = request.title.trim();
    if title.is_empty() {
        return Err("标题不能为空".to_string());
    }
    if !["movie", "series", "animation", "other", "unknown"].contains(&request.media_type.as_str())
    {
        return Err("无效的媒体类型".to_string());
    }

    let connection = lock_connection(&state)?;
    let affected = connection
        .execute(
            "UPDATE media_items
             SET title = ?2, sort_title = ?2, year = ?3, media_type = ?4,
                 overview = ?5, user_notes = ?6, watched = ?7,
                 recognition_status = 'manual', updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![
                request.id,
                title,
                request.year,
                request.media_type,
                clean_optional_text(request.overview),
                clean_optional_text(request.user_notes),
                request.watched
            ],
        )
        .map_err(|error| format!("无法保存影视资料: {error}"))?;

    if affected == 0 {
        return Err("影视条目不存在".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn set_watched_status(
    request: SetWatchedRequest,
    state: State<'_, DatabaseState>,
) -> Result<(), String> {
    let connection = lock_connection(&state)?;
    let affected = connection
        .execute(
            "UPDATE media_items
             SET watched = ?2, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![request.media_id, request.watched],
        )
        .map_err(|error| format!("无法更新观看状态: {error}"))?;
    if affected == 0 {
        return Err("影视条目不存在".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn set_media_type(
    request: SetMediaTypeRequest,
    state: State<'_, DatabaseState>,
) -> Result<(), String> {
    if !["movie", "series", "animation", "other"].contains(&request.media_type.as_str()) {
        return Err("无效的影视分类".to_string());
    }
    let connection = lock_connection(&state)?;
    let affected = connection
        .execute(
            "UPDATE media_items
             SET media_type = ?2, recognition_status = 'manual',
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![request.media_id, request.media_type],
        )
        .map_err(|error| format!("无法更新影视分类: {error}"))?;
    if affected == 0 {
        return Err("影视条目不存在".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn delete_media_items(
    request: DeleteMediaRequest,
    state: State<'_, DatabaseState>,
) -> Result<LibraryMutationResult, String> {
    let media_ids = unique_positive_ids(request.media_ids);
    if media_ids.is_empty() {
        return Err("没有选择要删除的影视条目".to_string());
    }

    let mut connection = lock_connection(&state)?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始删除影视条目: {error}"))?;
    let mut items_removed = 0_u64;
    for media_id in media_ids {
        transaction
            .execute(
                "INSERT OR IGNORE INTO media_blacklist
                    (path, file_name, media_title, scan_source_path)
                 SELECT file.path, file.file_name, item.title, source.path
                 FROM media_files file
                 JOIN media_items item ON item.id = file.media_item_id
                 LEFT JOIN scan_sources source ON source.id = file.scan_source_id
                 WHERE file.media_item_id = ?1",
                [media_id],
            )
            .map_err(|error| format!("无法将文件加入黑名单: {error}"))?;
        transaction
            .execute(
                "DELETE FROM media_files WHERE media_item_id = ?1",
                [media_id],
            )
            .map_err(|error| format!("无法移除媒体文件索引: {error}"))?;
        items_removed += transaction
            .execute("DELETE FROM media_items WHERE id = ?1", [media_id])
            .map_err(|error| format!("无法删除影视条目: {error}"))? as u64;
    }
    transaction
        .commit()
        .map_err(|error| format!("无法提交影视条目删除: {error}"))?;
    log::info!("removed {items_removed} media items from library");
    Ok(LibraryMutationResult {
        items_removed,
        files_relinked: 0,
    })
}

#[tauri::command]
pub fn list_blacklist(state: State<'_, DatabaseState>) -> Result<Vec<BlacklistItem>, String> {
    let connection = lock_connection(&state)?;
    let mut statement = connection
        .prepare(
            "SELECT id, path, file_name, media_title, scan_source_path, deleted_at
             FROM media_blacklist
             ORDER BY deleted_at DESC, id DESC",
        )
        .map_err(|error| format!("无法准备黑名单查询: {error}"))?;
    let items = statement
        .query_map([], |row| {
            Ok(BlacklistItem {
                id: row.get(0)?,
                path: row.get(1)?,
                file_name: row.get(2)?,
                media_title: row.get(3)?,
                scan_source_path: row.get(4)?,
                deleted_at: row.get(5)?,
            })
        })
        .map_err(|error| format!("无法查询黑名单: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取黑名单: {error}"))?;
    Ok(items)
}

#[tauri::command]
pub fn restore_blacklist_items(
    request: RestoreBlacklistRequest,
    state: State<'_, DatabaseState>,
) -> Result<u64, String> {
    let ids = unique_positive_ids(request.ids);
    if ids.is_empty() {
        return Err("没有选择要恢复的黑名单文件".to_string());
    }
    let connection = lock_connection(&state)?;
    let mut restored = 0_u64;
    for id in ids {
        restored += connection
            .execute("DELETE FROM media_blacklist WHERE id = ?1", [id])
            .map_err(|error| format!("无法恢复黑名单文件: {error}"))? as u64;
    }
    log::info!("restored {restored} files from blacklist");
    Ok(restored)
}

#[tauri::command]
pub fn clear_blacklist(state: State<'_, DatabaseState>) -> Result<u64, String> {
    let connection = lock_connection(&state)?;
    let restored = connection
        .execute("DELETE FROM media_blacklist", [])
        .map_err(|error| format!("无法清空黑名单: {error}"))? as u64;
    log::info!("cleared {restored} files from blacklist");
    Ok(restored)
}

#[tauri::command]
pub fn merge_media_items(
    request: MergeMediaRequest,
    state: State<'_, DatabaseState>,
) -> Result<LibraryMutationResult, String> {
    let media_ids = unique_positive_ids(request.media_ids);
    if media_ids.len() < 2 || !media_ids.contains(&request.keeper_id) {
        return Err("合并至少需要两个条目，并指定其中一个主条目".to_string());
    }

    let mut connection = lock_connection(&state)?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始合并影视条目: {error}"))?;
    let keeper_exists: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM media_items WHERE id = ?1)",
            [request.keeper_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("无法检查主条目: {error}"))?;
    if !keeper_exists {
        return Err("指定的主条目不存在".to_string());
    }

    let mut result = LibraryMutationResult {
        items_removed: 0,
        files_relinked: 0,
    };
    for duplicate_id in media_ids.into_iter().filter(|id| *id != request.keeper_id) {
        result.files_relinked += transaction
            .execute(
                "UPDATE media_files SET media_item_id = ?1 WHERE media_item_id = ?2",
                params![request.keeper_id, duplicate_id],
            )
            .map_err(|error| format!("无法重新关联媒体文件: {error}"))?
            as u64;
        transaction
            .execute(
                "INSERT OR IGNORE INTO media_tags (media_item_id, tag_id)
                 SELECT ?1, tag_id FROM media_tags WHERE media_item_id = ?2",
                params![request.keeper_id, duplicate_id],
            )
            .map_err(|error| format!("无法合并标签: {error}"))?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO collection_items
                    (collection_id, media_item_id, position)
                 SELECT collection_id, ?1, position
                 FROM collection_items WHERE media_item_id = ?2",
                params![request.keeper_id, duplicate_id],
            )
            .map_err(|error| format!("无法合并片单: {error}"))?;
        transaction
            .execute(
                "UPDATE artwork SET media_item_id = ?1, is_primary = 0
                 WHERE media_item_id = ?2",
                params![request.keeper_id, duplicate_id],
            )
            .map_err(|error| format!("无法合并图片: {error}"))?;
        transaction
            .execute(
                "UPDATE external_metadata SET media_item_id = ?1
                 WHERE media_item_id = ?2",
                params![request.keeper_id, duplicate_id],
            )
            .map_err(|error| format!("无法合并元数据来源: {error}"))?;
        result.items_removed += transaction
            .execute("DELETE FROM media_items WHERE id = ?1", [duplicate_id])
            .map_err(|error| format!("无法删除已合并条目: {error}"))?
            as u64;
    }
    transaction
        .execute(
            "UPDATE media_items
             SET recognition_status = 'manual', updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            [request.keeper_id],
        )
        .map_err(|error| format!("无法锁定主条目: {error}"))?;
    transaction
        .execute(
            "UPDATE artwork
             SET is_primary = CASE
                 WHEN id = (
                     SELECT MIN(id) FROM artwork
                     WHERE media_item_id = ?1 AND artwork_type = 'poster'
                 ) THEN 1 ELSE 0 END
             WHERE media_item_id = ?1 AND artwork_type = 'poster'
               AND NOT EXISTS (
                   SELECT 1 FROM artwork
                   WHERE media_item_id = ?1 AND artwork_type = 'poster'
                     AND is_primary = 1
               )",
            [request.keeper_id],
        )
        .map_err(|error| format!("无法整理合并后的主海报: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("无法提交影视条目合并: {error}"))?;
    log::info!(
        "manual merge completed: keeper={}, removed={}, files={}",
        request.keeper_id,
        result.items_removed,
        result.files_relinked
    );
    Ok(result)
}

#[tauri::command]
pub fn set_media_poster(
    app: AppHandle,
    media_id: i64,
    source_path: String,
    state: State<'_, DatabaseState>,
) -> Result<String, String> {
    let source = PathBuf::from(source_path);
    if !source.is_file() {
        return Err("选择的海报文件不存在".to_string());
    }
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "海报文件没有扩展名".to_string())?;
    if !["jpg", "jpeg", "png", "webp"].contains(&extension.as_str()) {
        return Err("仅支持 JPG、PNG 和 WebP 海报".to_string());
    }

    let poster_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| format!("无法确定应用数据目录: {error}"))?
        .join("cache")
        .join("posters");
    fs::create_dir_all(&poster_dir).map_err(|error| format!("无法创建海报缓存目录: {error}"))?;
    let destination = poster_dir.join(format!("{media_id}.{extension}"));
    fs::copy(&source, &destination).map_err(|error| format!("无法缓存海报: {error}"))?;
    let destination_text = destination.to_string_lossy().into_owned();

    let mut connection = lock_connection(&state)?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始海报更新: {error}"))?;
    transaction
        .execute(
            "UPDATE artwork SET is_primary = 0
             WHERE media_item_id = ?1 AND artwork_type = 'poster'",
            [media_id],
        )
        .map_err(|error| format!("无法更新旧海报: {error}"))?;
    transaction
        .execute(
            "INSERT INTO artwork (media_item_id, artwork_type, local_path, is_primary)
             VALUES (?1, 'poster', ?2, 1)",
            params![media_id, destination_text],
        )
        .map_err(|error| format!("无法保存海报记录: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("无法提交海报更新: {error}"))?;

    Ok(destination_text)
}

#[tauri::command]
pub fn list_tags(state: State<'_, DatabaseState>) -> Result<Vec<Tag>, String> {
    let connection = lock_connection(&state)?;
    let mut statement = connection
        .prepare("SELECT id, name, color FROM tags ORDER BY name COLLATE NOCASE")
        .map_err(|error| format!("无法准备标签查询: {error}"))?;
    let items = statement
        .query_map([], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
            })
        })
        .map_err(|error| format!("无法查询标签: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取标签: {error}"))?;
    Ok(items)
}

#[tauri::command]
pub fn create_tag(
    request: CreateTagRequest,
    state: State<'_, DatabaseState>,
) -> Result<Tag, String> {
    let name = request.name.trim();
    if name.is_empty() {
        return Err("标签名称不能为空".to_string());
    }
    let color = clean_optional_text(request.color);
    let connection = lock_connection(&state)?;
    connection
        .execute(
            "INSERT INTO tags (name, color) VALUES (?1, ?2)",
            params![name, color],
        )
        .map_err(|error| format!("无法创建标签，名称可能已存在: {error}"))?;
    Ok(Tag {
        id: connection.last_insert_rowid(),
        name: name.to_string(),
        color,
    })
}

#[tauri::command]
pub fn set_media_tags(
    request: SetMediaRelationsRequest,
    state: State<'_, DatabaseState>,
) -> Result<(), String> {
    let mut connection = lock_connection(&state)?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始标签更新: {error}"))?;
    transaction
        .execute(
            "DELETE FROM media_tags WHERE media_item_id = ?1",
            [request.media_id],
        )
        .map_err(|error| format!("无法清理旧标签: {error}"))?;
    for tag_id in request.ids {
        transaction
            .execute(
                "INSERT OR IGNORE INTO media_tags (media_item_id, tag_id) VALUES (?1, ?2)",
                params![request.media_id, tag_id],
            )
            .map_err(|error| format!("无法关联标签: {error}"))?;
    }
    transaction
        .commit()
        .map_err(|error| format!("无法提交标签更新: {error}"))
}

#[tauri::command]
pub fn list_collections(state: State<'_, DatabaseState>) -> Result<Vec<Collection>, String> {
    let connection = lock_connection(&state)?;
    let mut statement = connection
        .prepare("SELECT id, name, description FROM collections ORDER BY name COLLATE NOCASE")
        .map_err(|error| format!("无法准备片单查询: {error}"))?;
    let items = statement
        .query_map([], |row| {
            Ok(Collection {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
            })
        })
        .map_err(|error| format!("无法查询片单: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取片单: {error}"))?;
    Ok(items)
}

#[tauri::command]
pub fn create_collection(
    request: CreateCollectionRequest,
    state: State<'_, DatabaseState>,
) -> Result<Collection, String> {
    let name = request.name.trim();
    if name.is_empty() {
        return Err("片单名称不能为空".to_string());
    }
    let description = clean_optional_text(request.description);
    let connection = lock_connection(&state)?;
    connection
        .execute(
            "INSERT INTO collections (name, description) VALUES (?1, ?2)",
            params![name, description],
        )
        .map_err(|error| format!("无法创建片单，名称可能已存在: {error}"))?;
    Ok(Collection {
        id: connection.last_insert_rowid(),
        name: name.to_string(),
        description,
    })
}

#[tauri::command]
pub fn set_media_collections(
    request: SetMediaRelationsRequest,
    state: State<'_, DatabaseState>,
) -> Result<(), String> {
    let mut connection = lock_connection(&state)?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始片单更新: {error}"))?;
    transaction
        .execute(
            "DELETE FROM collection_items WHERE media_item_id = ?1",
            [request.media_id],
        )
        .map_err(|error| format!("无法清理旧片单关系: {error}"))?;
    for (position, collection_id) in request.ids.into_iter().enumerate() {
        transaction
            .execute(
                "INSERT OR IGNORE INTO collection_items
                 (collection_id, media_item_id, position) VALUES (?1, ?2, ?3)",
                params![collection_id, request.media_id, position as i64],
            )
            .map_err(|error| format!("无法加入片单: {error}"))?;
    }
    transaction
        .commit()
        .map_err(|error| format!("无法提交片单更新: {error}"))
}

#[tauri::command]
pub fn list_scan_history(state: State<'_, DatabaseState>) -> Result<Vec<ScanHistoryItem>, String> {
    let connection = lock_connection(&state)?;
    let mut statement = connection
        .prepare(
            "SELECT history.id, source.display_name, history.started_at,
                    history.finished_at, history.status, history.files_found,
                    history.files_added, history.files_updated,
                    history.files_missing, history.files_ignored, history.error_message
             FROM scan_history history
             LEFT JOIN scan_sources source ON source.id = history.scan_source_id
             ORDER BY history.id DESC LIMIT 50",
        )
        .map_err(|error| format!("无法准备扫描历史查询: {error}"))?;
    let items = statement
        .query_map([], |row| {
            Ok(ScanHistoryItem {
                id: row.get(0)?,
                source_name: row.get(1)?,
                started_at: row.get(2)?,
                finished_at: row.get(3)?,
                status: row.get(4)?,
                files_found: row.get(5)?,
                files_added: row.get(6)?,
                files_updated: row.get(7)?,
                files_missing: row.get(8)?,
                files_ignored: row.get(9)?,
                error_message: row.get(10)?,
            })
        })
        .map_err(|error| format!("无法查询扫描历史: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取扫描历史: {error}"))?;
    Ok(items)
}

#[tauri::command]
pub fn merge_duplicate_media(
    state: State<'_, DatabaseState>,
) -> Result<MergeDuplicatesResult, String> {
    let mut connection = lock_connection(&state)?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始重复条目整理: {error}"))?;

    {
        let mut statement = transaction
            .prepare(
                "SELECT file.id, file.media_item_id, file.path, source.path
                 FROM media_files file
                 JOIN scan_sources source ON source.id = file.scan_source_id
                 WHERE file.media_item_id IS NOT NULL",
            )
            .map_err(|error| format!("无法读取待整理文件: {error}"))?;
        let files = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|error| format!("无法查询待整理文件: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("无法解析待整理文件: {error}"))?;
        drop(statement);

        for (file_id, media_id, path, source_path) in files {
            let parsed = parse_media_name(Path::new(&path), Path::new(&source_path));
            transaction
                .execute(
                    "UPDATE media_files
                     SET season_number = ?2, episode_number = ?3
                     WHERE id = ?1",
                    params![file_id, parsed.season_number, parsed.episode_number],
                )
                .map_err(|error| format!("无法更新剧集编号: {error}"))?;
            transaction
                .execute(
                    "UPDATE media_items
                     SET title = ?2, sort_title = ?2, year = ?3, media_type = ?4,
                         group_key = ?5, season_number = NULL, episode_number = NULL,
                         updated_at = CURRENT_TIMESTAMP
                     WHERE id = ?1 AND recognition_status != 'manual'",
                    params![
                        media_id,
                        parsed.title,
                        parsed.year,
                        parsed.media_type,
                        parsed.group_key
                    ],
                )
                .map_err(|error| format!("无法更新影视分组信息: {error}"))?;
        }
    }

    let manual_items = {
        let mut statement = transaction
            .prepare(
                "SELECT id, title FROM media_items
                 WHERE recognition_status = 'manual'
                   AND (group_key IS NULL OR group_key = '')",
            )
            .map_err(|error| format!("无法读取手工条目: {error}"))?;
        let items = statement
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| format!("无法查询手工条目: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("无法解析手工条目: {error}"))?;
        items
    };
    for (id, title) in manual_items {
        transaction
            .execute(
                "UPDATE media_items SET group_key = ?2 WHERE id = ?1",
                params![id, normalized_group_key(&title)],
            )
            .map_err(|error| format!("无法更新手工条目分组: {error}"))?;
    }

    let candidates = {
        let mut statement = transaction
            .prepare(
                "SELECT item.id, item.media_type, item.group_key, item.year,
                        item.recognition_status, COUNT(file.id)
                 FROM media_items item
                 LEFT JOIN media_files file ON file.media_item_id = item.id
                 WHERE item.group_key IS NOT NULL AND item.group_key != ''
                 GROUP BY item.id
                 ORDER BY (item.recognition_status = 'manual') DESC,
                          COUNT(file.id) DESC, item.id ASC",
            )
            .map_err(|error| format!("无法读取重复候选条目: {error}"))?;
        let items = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i32>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|error| format!("无法查询重复候选条目: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("无法解析重复候选条目: {error}"))?;
        items
    };

    let mut groups: HashMap<MediaGroupKey, Vec<MediaGroupMember>> = HashMap::new();
    for (id, media_type, group_key, year, recognition_status) in candidates {
        let group_year = match media_type.as_str() {
            "series" | "animation" => None,
            "movie" if year.is_some() => year,
            _ => continue,
        };
        groups
            .entry((media_type, group_key, group_year))
            .or_default()
            .push((id, recognition_status));
    }

    let mut result = MergeDuplicatesResult {
        groups_merged: 0,
        items_removed: 0,
        files_relinked: 0,
    };
    for items in groups.into_values().filter(|items| items.len() > 1) {
        if items
            .iter()
            .filter(|(_, status)| status == "manual")
            .count()
            > 1
        {
            continue;
        }
        let keeper_id = items[0].0;
        for (duplicate_id, _) in items.into_iter().skip(1) {
            let moved = transaction
                .execute(
                    "UPDATE media_files SET media_item_id = ?1 WHERE media_item_id = ?2",
                    params![keeper_id, duplicate_id],
                )
                .map_err(|error| format!("无法重新关联媒体文件: {error}"))?;
            transaction
                .execute(
                    "INSERT OR IGNORE INTO media_tags (media_item_id, tag_id)
                     SELECT ?1, tag_id FROM media_tags WHERE media_item_id = ?2",
                    params![keeper_id, duplicate_id],
                )
                .map_err(|error| format!("无法合并标签: {error}"))?;
            transaction
                .execute(
                    "INSERT OR IGNORE INTO collection_items
                     (collection_id, media_item_id, position)
                     SELECT collection_id, ?1, position
                     FROM collection_items WHERE media_item_id = ?2",
                    params![keeper_id, duplicate_id],
                )
                .map_err(|error| format!("无法合并片单: {error}"))?;
            transaction
                .execute(
                    "UPDATE artwork SET media_item_id = ?1, is_primary = 0
                     WHERE media_item_id = ?2",
                    params![keeper_id, duplicate_id],
                )
                .map_err(|error| format!("无法合并图片记录: {error}"))?;
            transaction
                .execute(
                    "UPDATE external_metadata SET media_item_id = ?1
                     WHERE media_item_id = ?2",
                    params![keeper_id, duplicate_id],
                )
                .map_err(|error| format!("无法合并外部资料: {error}"))?;
            transaction
                .execute("DELETE FROM media_items WHERE id = ?1", [duplicate_id])
                .map_err(|error| format!("无法删除重复条目: {error}"))?;
            result.items_removed += 1;
            result.files_relinked += moved as u64;
        }
        transaction
            .execute(
                "UPDATE artwork
                 SET is_primary = CASE
                     WHEN id = (
                         SELECT MIN(id) FROM artwork
                         WHERE media_item_id = ?1 AND artwork_type = 'poster'
                     ) THEN 1 ELSE 0 END
                 WHERE media_item_id = ?1 AND artwork_type = 'poster'
                   AND NOT EXISTS (
                       SELECT 1 FROM artwork
                       WHERE media_item_id = ?1 AND artwork_type = 'poster'
                         AND is_primary = 1
                   )",
                [keeper_id],
            )
            .map_err(|error| format!("无法整理主海报: {error}"))?;
        result.groups_merged += 1;
    }

    transaction
        .commit()
        .map_err(|error| format!("无法提交重复条目整理: {error}"))?;
    log::info!(
        "duplicate merge completed: groups={}, removed={}, files={}",
        result.groups_merged,
        result.items_removed,
        result.files_relinked
    );
    Ok(result)
}

pub(crate) fn lock_connection<'a>(
    state: &'a State<'_, DatabaseState>,
) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    state
        .connection
        .lock()
        .map_err(|_| "数据库连接锁已损坏".to_string())
}

fn count_rows(connection: &Connection, table: &str) -> Result<i64, String> {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .map_err(|error| format!("无法统计 {table}: {error}"))
}

fn split_i64_list(value: Option<String>) -> Vec<i64> {
    value
        .as_deref()
        .unwrap_or_default()
        .split('|')
        .filter_map(|item| item.parse().ok())
        .collect()
}

fn split_string_list(value: Option<String>) -> Vec<String> {
    value
        .as_deref()
        .unwrap_or_default()
        .split('|')
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn restore_local_tmdb_token(connection: &Connection, token: Option<&str>) -> Result<(), String> {
    let Some(token) = token.filter(|value| !value.trim().is_empty()) else {
        return Ok(());
    };
    connection
        .execute(
            "INSERT INTO app_settings (key, value)
             VALUES ('tmdb_read_access_token', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [token],
        )
        .map_err(|error| format!("无法恢复本机 TMDB Token: {error}"))?;
    Ok(())
}

fn create_backup_file(
    connection: &Connection,
    app_version: &str,
    destination: &Path,
) -> Result<BackupResult, String> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| format!("无法创建备份目录: {error}"))?;
    let temporary = parent.join(format!(
        ".mediamanager-backup-{}-{}.tmp",
        std::process::id(),
        unix_timestamp()
    ));
    let _ = fs::remove_file(&temporary);
    connection
        .execute(
            "VACUUM main INTO ?1",
            [temporary.to_string_lossy().as_ref()],
        )
        .map_err(|error| format!("无法创建 SQLite 一致性快照: {error}"))?;

    let backup_result = (|| {
        let mut backup =
            Connection::open(&temporary).map_err(|error| format!("无法打开备份快照: {error}"))?;
        backup
            .execute_batch(
                "DROP TABLE IF EXISTS backup_artwork_files;
                 DROP TABLE IF EXISTS backup_manifest;
                 CREATE TABLE backup_manifest (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                 );
                 CREATE TABLE backup_artwork_files (
                    artwork_id INTEGER PRIMARY KEY,
                    file_name TEXT NOT NULL,
                    contents BLOB NOT NULL
                 );",
            )
            .map_err(|error| format!("无法创建备份清单: {error}"))?;
        let schema_version: i64 = backup
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(|error| format!("无法读取备份数据库版本: {error}"))?;
        backup
            .execute(
                "INSERT INTO backup_manifest (key, value) VALUES
                    ('format_version', '1'),
                    ('schema_version', ?1),
                    ('app_version', ?2),
                    ('created_at_unix', ?3),
                    ('includes_secrets', ?4)",
                params![
                    schema_version.to_string(),
                    app_version,
                    unix_timestamp().to_string(),
                    "false"
                ],
            )
            .map_err(|error| format!("无法写入备份清单: {error}"))?;
        backup
            .execute(
                "DELETE FROM app_settings WHERE key = 'tmdb_read_access_token'",
                [],
            )
            .map_err(|error| format!("无法从备份中排除访问令牌: {error}"))?;

        let artwork_rows = {
            let mut statement = backup
                .prepare(
                    "SELECT id, local_path FROM artwork
                     WHERE local_path IS NOT NULL AND TRIM(local_path) <> ''",
                )
                .map_err(|error| format!("无法读取待备份海报: {error}"))?;
            let rows = statement
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|error| format!("无法查询待备份海报: {error}"))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("无法整理待备份海报: {error}"))?;
            rows
        };
        let transaction = backup
            .transaction()
            .map_err(|error| format!("无法开始海报备份: {error}"))?;
        let mut artwork_files = 0_u64;
        for (artwork_id, local_path) in artwork_rows {
            let source = PathBuf::from(&local_path);
            if !source.is_file() {
                continue;
            }
            let contents =
                fs::read(&source).map_err(|error| format!("无法读取海报 {local_path}: {error}"))?;
            let extension = source
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("jpg");
            let file_name = format!("{artwork_id}.{extension}");
            transaction
                .execute(
                    "INSERT INTO backup_artwork_files (artwork_id, file_name, contents)
                     VALUES (?1, ?2, ?3)",
                    params![artwork_id, file_name, contents],
                )
                .map_err(|error| format!("无法写入备份海报: {error}"))?;
            artwork_files += 1;
        }
        transaction
            .commit()
            .map_err(|error| format!("无法提交海报备份: {error}"))?;
        backup
            .execute_batch("VACUUM;")
            .map_err(|error| format!("无法压缩备份文件: {error}"))?;
        drop(backup);

        if destination.exists() {
            fs::remove_file(destination)
                .map_err(|error| format!("无法覆盖现有备份文件: {error}"))?;
        }
        if let Err(rename_error) = fs::rename(&temporary, destination) {
            fs::copy(&temporary, destination)
                .map_err(|copy_error| format!("无法保存备份: {rename_error}; {copy_error}"))?;
            fs::remove_file(&temporary).map_err(|error| format!("无法清理临时备份: {error}"))?;
        }
        let size_bytes = fs::metadata(destination)
            .map_err(|error| format!("无法读取备份文件大小: {error}"))?
            .len();
        Ok(BackupResult {
            path: destination.to_string_lossy().into_owned(),
            size_bytes,
            artwork_files,
        })
    })();
    if backup_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    backup_result
}

fn backup_manifest_value(connection: &Connection, key: &str) -> Result<String, String> {
    connection
        .query_row(
            "SELECT value FROM backup_manifest WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .map_err(|error| format!("无效的 MediaManager 备份（缺少 {key}）: {error}"))
}

fn restore_embedded_artwork(
    connection: &mut Connection,
    staging_dir: &Path,
    final_dir: &Path,
) -> Result<u64, String> {
    let rows = {
        let mut statement = connection
            .prepare(
                "SELECT artwork_id, file_name, contents
                 FROM backup_artwork_files ORDER BY artwork_id",
            )
            .map_err(|error| format!("无法读取备份海报清单: {error}"))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })
            .map_err(|error| format!("无法查询备份海报: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("无法整理备份海报: {error}"))?;
        rows
    };
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始恢复海报: {error}"))?;
    let mut restored = 0_u64;
    for (artwork_id, file_name, contents) in rows {
        let safe_name = Path::new(&file_name)
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "备份中包含无效海报文件名".to_string())?;
        let staged = staging_dir.join(safe_name);
        let destination = final_dir.join(safe_name);
        fs::write(&staged, contents)
            .map_err(|error| format!("无法暂存恢复海报 {}: {error}", staged.display()))?;
        transaction
            .execute(
                "UPDATE artwork SET local_path = ?2 WHERE id = ?1",
                params![artwork_id, destination.to_string_lossy().as_ref()],
            )
            .map_err(|error| format!("无法更新恢复海报路径: {error}"))?;
        restored += 1;
    }
    transaction
        .commit()
        .map_err(|error| format!("无法提交海报恢复: {error}"))?;
    Ok(restored)
}

fn copy_staged_artwork(staging_dir: &Path, poster_dir: &Path) -> Result<(), String> {
    for entry in
        fs::read_dir(staging_dir).map_err(|error| format!("无法读取海报恢复暂存目录: {error}"))?
    {
        let entry = entry.map_err(|error| format!("无法读取暂存海报: {error}"))?;
        if !entry
            .file_type()
            .map_err(|error| format!("无法读取暂存海报类型: {error}"))?
            .is_file()
        {
            continue;
        }
        let destination = poster_dir.join(entry.file_name());
        fs::copy(entry.path(), &destination)
            .map_err(|error| format!("无法写入恢复海报 {}: {error}", destination.display()))?;
    }
    Ok(())
}

fn copy_backup_database(source: &Path, destination: &Path) -> Result<(), String> {
    remove_sqlite_files(destination)?;
    fs::copy(source, destination).map_err(|error| format!("无法复制自动备份进行回滚: {error}"))?;
    Ok(())
}

fn remove_sqlite_files(path: &Path) -> Result<(), String> {
    for target in [
        path.to_path_buf(),
        path_with_suffix(path, "-wal"),
        path_with_suffix(path, "-shm"),
    ] {
        if target.exists() {
            fs::remove_file(&target)
                .map_err(|error| format!("无法移除旧数据库文件 {}: {error}", target.display()))?;
        }
    }
    Ok(())
}

fn path_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn normalize_root(value: &str) -> Result<String, String> {
    let value = value.trim().replace('/', "\\");
    let value = value.trim_end_matches('\\');
    if value.is_empty() {
        return Err("媒体根路径不能为空".to_string());
    }
    Ok(value.to_string())
}

fn migrate_table_paths(
    transaction: &rusqlite::Transaction<'_>,
    table: &str,
    column: &str,
    old_root: &str,
    new_root: &str,
) -> Result<u64, String> {
    let query = format!("SELECT id, {column} FROM {table} WHERE {column} IS NOT NULL");
    let rows = {
        let mut statement = transaction
            .prepare(&query)
            .map_err(|error| format!("无法准备路径迁移查询: {error}"))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| format!("无法查询待迁移路径: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("无法整理待迁移路径: {error}"))?;
        rows
    };
    let update = format!("UPDATE {table} SET {column} = ?2 WHERE id = ?1");
    let mut updated = 0_u64;
    for (id, current) in rows {
        let Some(replacement) = replace_path_root(&current, old_root, new_root) else {
            continue;
        };
        updated += transaction
            .execute(&update, params![id, replacement])
            .map_err(|error| format!("无法迁移路径 {current}: {error}"))? as u64;
    }
    Ok(updated)
}

fn replace_path_root(path: &str, old_root: &str, new_root: &str) -> Option<String> {
    if path.len() < old_root.len() {
        return None;
    }
    let prefix = path.get(..old_root.len())?;
    if !prefix.eq_ignore_ascii_case(old_root) {
        return None;
    }
    let suffix = path.get(old_root.len()..)?;
    if !suffix.is_empty() && !suffix.starts_with(['\\', '/']) {
        return None;
    }
    Some(format!("{new_root}{}", suffix.replace('/', "\\")))
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn clean_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn unique_positive_ids(ids: Vec<i64>) -> Vec<i64> {
    let mut ids = ids.into_iter().filter(|id| *id > 0).collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_all_migrations_to_new_database() {
        let path =
            std::env::temp_dir().join(format!("media-manager-test-{}.db", std::process::id()));
        let _ = fs::remove_file(&path);

        let connection = open_database(&path).expect("database should migrate");
        let version: i64 = connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("schema version should exist");
        let season_column: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('media_items')
                 WHERE name = 'season_number'",
                [],
                |row| row.get(0),
            )
            .expect("season column query should work");

        let group_column: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('media_items')
                 WHERE name = 'group_key'",
                [],
                |row| row.get(0),
            )
            .expect("group key column query should work");

        let settings_table: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'app_settings'",
                [],
                |row| row.get(0),
            )
            .expect("settings table query should work");
        let blacklist_table: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'media_blacklist'",
                [],
                |row| row.get(0),
            )
            .expect("blacklist table query should work");

        assert_eq!(version, 5);
        assert_eq!(season_column, 1);
        assert_eq!(group_column, 1);
        assert_eq!(settings_table, 1);
        assert_eq!(blacklist_table, 1);
        drop(connection);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn creates_self_contained_backup_without_secrets() {
        let temp = std::env::temp_dir();
        let suffix = format!("{}-{}", std::process::id(), unix_timestamp());
        let database_path = temp.join(format!("media-manager-backup-source-{suffix}.db"));
        let poster_path = temp.join(format!("media-manager-backup-poster-{suffix}.jpg"));
        let backup_path = temp.join(format!("media-manager-backup-{suffix}.mmbak"));
        for path in [&database_path, &poster_path, &backup_path] {
            let _ = fs::remove_file(path);
        }
        fs::write(&poster_path, b"poster-bytes").expect("write poster");

        let connection = open_database(&database_path).expect("open source database");
        connection
            .execute_batch(
                "INSERT INTO scan_sources (path) VALUES ('A:\\Media');
                 INSERT INTO media_items (title) VALUES ('Backup Test');
                 INSERT INTO artwork
                    (media_item_id, artwork_type, local_path, is_primary)
                 VALUES (1, 'poster', '', 1);
                 INSERT INTO app_settings (key, value)
                 VALUES ('tmdb_read_access_token', 'secret');",
            )
            .expect("seed backup database");
        connection
            .execute(
                "UPDATE artwork SET local_path = ?1 WHERE id = 1",
                [poster_path.to_string_lossy().as_ref()],
            )
            .expect("set poster path");

        let result = create_backup_file(&connection, "0.1.0", &backup_path).expect("create backup");
        assert_eq!(result.artwork_files, 1);

        let mut backup = Connection::open(&backup_path).expect("open backup");
        assert_eq!(
            backup_manifest_value(&backup, "format_version").expect("format version"),
            "1"
        );
        assert_eq!(
            backup_manifest_value(&backup, "includes_secrets").expect("secret marker"),
            "false"
        );
        let artwork_count: i64 = backup
            .query_row("SELECT COUNT(*) FROM backup_artwork_files", [], |row| {
                row.get(0)
            })
            .expect("count artwork");
        let secret_count: i64 = backup
            .query_row(
                "SELECT COUNT(*) FROM app_settings
                 WHERE key = 'tmdb_read_access_token'",
                [],
                |row| row.get(0),
            )
            .expect("count secrets");
        assert_eq!(artwork_count, 1);
        assert_eq!(secret_count, 0);

        let restore_dir = temp.join(format!("media-manager-restore-posters-{suffix}"));
        let _ = fs::remove_dir_all(&restore_dir);
        fs::create_dir_all(&restore_dir).expect("create restore directory");
        assert_eq!(
            restore_embedded_artwork(&mut backup, &restore_dir, &restore_dir)
                .expect("restore artwork"),
            1
        );
        let restored_path: String = backup
            .query_row("SELECT local_path FROM artwork WHERE id = 1", [], |row| {
                row.get(0)
            })
            .expect("restored artwork path");
        assert_eq!(
            fs::read(restored_path).expect("read restored artwork"),
            b"poster-bytes"
        );

        drop(backup);
        drop(connection);
        for path in [database_path, poster_path, backup_path] {
            let _ = fs::remove_file(path);
        }
        let _ = fs::remove_dir_all(restore_dir);
    }

    #[test]
    fn replaces_media_root_only_on_path_boundary() {
        assert_eq!(
            replace_path_root(r"A:\Media\Anime\Episode01.mkv", r"a:\media", r"E:\Library"),
            Some(r"E:\Library\Anime\Episode01.mkv".to_string())
        );
        assert_eq!(
            replace_path_root(r"A:\Media-Old\Movie.mkv", r"A:\Media", r"E:\Library"),
            None
        );
    }
}
