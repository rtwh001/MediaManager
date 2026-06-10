use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use walkdir::WalkDir;

use crate::{
    database::DatabaseState,
    parser::{parse_media_name, ParsedMediaName},
    probe::{probe_media, MediaProbe},
};

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi", "mov", "wmv", "m4v", "ts"];

#[derive(Default)]
pub struct ScannerState {
    running: Arc<AtomicBool>,
    cancel_requested: Arc<AtomicBool>,
}

impl ScannerState {
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    running: bool,
    cancelled: bool,
    current_source: Option<String>,
    current_file: Option<String>,
    files_found: u64,
    files_ignored: u64,
    files_added: u64,
    files_updated: u64,
    files_missing: u64,
    errors: u64,
}

pub type ScanSummary = ScanProgress;

struct ScanSourceRecord {
    id: i64,
    path: PathBuf,
    display_name: String,
    recursive: bool,
}

enum UpsertOutcome {
    Added,
    Updated,
    Unchanged,
}

#[tauri::command]
pub async fn scan_library(
    app: AppHandle,
    database: State<'_, DatabaseState>,
    scanner: State<'_, ScannerState>,
) -> Result<ScanSummary, String> {
    if scanner.running.swap(true, Ordering::SeqCst) {
        return Err("扫描任务已经在运行".to_string());
    }

    log::info!("library scan requested");
    scanner.cancel_requested.store(false, Ordering::SeqCst);
    let database_path = database.path().to_path_buf();
    let running = Arc::clone(&scanner.running);
    let cancel_requested = Arc::clone(&scanner.cancel_requested);

    let join_result = tauri::async_runtime::spawn_blocking(move || {
        scan_database(database_path, &app, cancel_requested)
    })
    .await;
    running.store(false, Ordering::SeqCst);
    let result = join_result.map_err(|error| format!("扫描任务意外终止: {error}"))?;
    match &result {
        Ok(summary) => log::info!(
            "library scan finished: found={}, ignored={}, added={}, updated={}, missing={}, errors={}, cancelled={}",
            summary.files_found,
            summary.files_ignored,
            summary.files_added,
            summary.files_updated,
            summary.files_missing,
            summary.errors,
            summary.cancelled
        ),
        Err(error) => log::error!("library scan failed: {error}"),
    }
    result
}

#[tauri::command]
pub fn cancel_scan(scanner: State<'_, ScannerState>) -> bool {
    if scanner.running.load(Ordering::SeqCst) {
        scanner.cancel_requested.store(true, Ordering::SeqCst);
        true
    } else {
        false
    }
}

#[tauri::command]
pub fn scan_running(scanner: State<'_, ScannerState>) -> bool {
    scanner.is_running()
}

fn scan_database(
    database_path: PathBuf,
    app: &AppHandle,
    cancel_requested: Arc<AtomicBool>,
) -> Result<ScanSummary, String> {
    let mut connection =
        Connection::open(&database_path).map_err(|error| format!("无法打开扫描数据库: {error}"))?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(|error| format!("无法配置扫描数据库连接: {error}"))?;

    let sources = load_sources(&connection)?;
    let mut progress = ScanProgress {
        running: true,
        cancelled: false,
        current_source: None,
        current_file: None,
        files_found: 0,
        files_ignored: 0,
        files_added: 0,
        files_updated: 0,
        files_missing: 0,
        errors: 0,
    };
    emit_progress(app, &progress);

    for source in sources {
        if cancel_requested.load(Ordering::SeqCst) {
            progress.cancelled = true;
            break;
        }
        progress.current_source = Some(source.display_name.clone());
        scan_source(
            &mut connection,
            &source,
            &mut progress,
            app,
            &cancel_requested,
        )?;
    }

    progress.running = false;
    progress.current_file = None;
    emit_progress(app, &progress);
    Ok(progress)
}

fn load_sources(connection: &Connection) -> Result<Vec<ScanSourceRecord>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, path, COALESCE(display_name, path), recursive
             FROM scan_sources WHERE enabled = 1 ORDER BY id",
        )
        .map_err(|error| format!("无法读取扫描目录: {error}"))?;

    let sources = statement
        .query_map([], |row| {
            Ok(ScanSourceRecord {
                id: row.get(0)?,
                path: PathBuf::from(row.get::<_, String>(1)?),
                display_name: row.get(2)?,
                recursive: row.get(3)?,
            })
        })
        .map_err(|error| format!("无法查询扫描目录: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法解析扫描目录: {error}"))?;

    Ok(sources)
}

fn scan_source(
    connection: &mut Connection,
    source: &ScanSourceRecord,
    progress: &mut ScanProgress,
    app: &AppHandle,
    cancel_requested: &AtomicBool,
) -> Result<(), String> {
    let history_id = connection
        .query_row(
            "INSERT INTO scan_history (scan_source_id) VALUES (?1) RETURNING id",
            [source.id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("无法创建扫描记录: {error}"))?;

    connection
        .execute(
            "UPDATE media_files SET is_missing = 1 WHERE scan_source_id = ?1",
            [source.id],
        )
        .map_err(|error| format!("无法初始化文件状态: {error}"))?;

    if !source.path.is_dir() {
        finish_scan(
            connection,
            history_id,
            "failed",
            0,
            0,
            0,
            0,
            0,
            Some("扫描目录不存在或无法访问"),
        )?;
        progress.errors += 1;
        emit_progress(app, progress);
        return Ok(());
    }

    let start_found = progress.files_found;
    let start_ignored = progress.files_ignored;
    let start_added = progress.files_added;
    let start_updated = progress.files_updated;
    let start_errors = progress.errors;
    let mut error_messages = Vec::new();

    let mut walker = WalkDir::new(&source.path).follow_links(false);
    if !source.recursive {
        walker = walker.max_depth(1);
    }

    for entry in walker {
        if cancel_requested.load(Ordering::SeqCst) {
            progress.cancelled = true;
            break;
        }

        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                progress.errors += 1;
                log::warn!(
                    "walk directory error for {}: {error}",
                    source.path.display()
                );
                error_messages.push(error.to_string());
                continue;
            }
        };
        if !entry.file_type().is_file() || !is_supported_video(entry.path()) {
            continue;
        }

        progress.files_found += 1;
        progress.current_file = Some(entry.path().to_string_lossy().into_owned());
        if is_blacklisted(connection, entry.path())? {
            progress.files_ignored += 1;
            emit_progress(app, progress);
            continue;
        }
        match upsert_media_file(connection, source.id, &source.path, entry.path()) {
            Ok(UpsertOutcome::Added) => progress.files_added += 1,
            Ok(UpsertOutcome::Updated) => progress.files_updated += 1,
            Ok(UpsertOutcome::Unchanged) => {}
            Err(error) => {
                progress.errors += 1;
                log::warn!("media scan error for {}: {error}", entry.path().display());
                error_messages.push(format!("{}: {error}", entry.path().display()));
            }
        }
        emit_progress(app, progress);
    }

    let missing: u64 = if progress.cancelled {
        connection
            .execute(
                "UPDATE media_files SET is_missing = 0 WHERE scan_source_id = ?1",
                [source.id],
            )
            .map_err(|error| format!("无法恢复取消扫描的文件状态: {error}"))?;
        0
    } else {
        connection
            .query_row(
                "SELECT COUNT(*) FROM media_files WHERE scan_source_id = ?1 AND is_missing = 1",
                [source.id],
                |row| row.get(0),
            )
            .map_err(|error| format!("无法统计缺失文件: {error}"))?
    };
    progress.files_missing += missing;

    connection
        .execute(
            "UPDATE scan_sources
             SET last_scanned_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            [source.id],
        )
        .map_err(|error| format!("无法更新扫描目录状态: {error}"))?;

    let status = if progress.cancelled {
        "cancelled"
    } else {
        "completed"
    };
    let error_message = if error_messages.is_empty() {
        None
    } else {
        Some(error_messages.join("\n"))
    };

    finish_scan(
        connection,
        history_id,
        status,
        progress.files_found - start_found,
        progress.files_ignored - start_ignored,
        progress.files_added - start_added,
        progress.files_updated - start_updated,
        missing,
        error_message.as_deref(),
    )?;

    if progress.errors > start_errors {
        emit_progress(app, progress);
    }
    Ok(())
}

fn upsert_media_file(
    connection: &mut Connection,
    scan_source_id: i64,
    source_root: &Path,
    path: &Path,
) -> Result<UpsertOutcome, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("无法读取文件信息: {error}"))?;
    let path_text = path.to_string_lossy().into_owned();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "文件名不是有效文本".to_string())?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let modified_at = system_time_text(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH));
    let file_size = metadata.len() as i64;
    let fingerprint = format!("{file_size}:{modified_at}");
    let parsed = parse_media_name(path, source_root);

    let existing = connection
        .query_row(
            "SELECT id, media_item_id, file_size, modified_at, width
             FROM media_files WHERE path = ?1",
            [&path_text],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("无法查询已有文件: {error}"))?;

    if let Some((file_id, media_item_id, old_size, old_modified_at, old_width)) = existing {
        let changed = old_size != file_size || old_modified_at != modified_at;
        let probe = if changed || old_width.is_none() {
            probe_media(path).unwrap_or_default()
        } else {
            MediaProbe::default()
        };

        connection
            .execute(
                "UPDATE media_files
                 SET scan_source_id = ?2, file_name = ?3, extension = ?4,
                     file_size = ?5, modified_at = ?6, fingerprint = ?7,
                     duration_seconds = COALESCE(?8, duration_seconds),
                     width = COALESCE(?9, width), height = COALESCE(?10, height),
                     video_codec = COALESCE(?11, video_codec),
                     audio_codec = COALESCE(?12, audio_codec),
                     container_format = COALESCE(?13, container_format),
                     hdr_format = COALESCE(?14, hdr_format),
                     season_number = ?15, episode_number = ?16,
                     is_missing = 0, updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?1",
                params![
                    file_id,
                    scan_source_id,
                    file_name,
                    extension,
                    file_size,
                    modified_at,
                    fingerprint,
                    probe.duration_seconds,
                    probe.width,
                    probe.height,
                    probe.video_codec,
                    probe.audio_codec,
                    probe.container_format,
                    probe.hdr_format,
                    parsed.season_number,
                    parsed.episode_number,
                ],
            )
            .map_err(|error| format!("无法更新媒体文件: {error}"))?;

        if let Some(media_item_id) = media_item_id {
            update_parsed_media(connection, media_item_id, &parsed)?;
        }
        return Ok(if changed {
            UpsertOutcome::Updated
        } else {
            UpsertOutcome::Unchanged
        });
    }

    let probe = probe_media(path).unwrap_or_default();
    let media_item_id = match find_grouped_media_item(connection, &parsed)? {
        Some(id) => id,
        None => connection
            .query_row(
                "INSERT INTO media_items (
                    title, sort_title, year, media_type, recognition_status,
                    season_number, episode_number, group_key
                 ) VALUES (?1, ?1, ?2, ?3, 'recognized', NULL, NULL, ?4)
                 RETURNING id",
                params![
                    parsed.title,
                    parsed.year,
                    parsed.media_type,
                    parsed.group_key
                ],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| format!("无法创建影视条目: {error}"))?,
    };

    connection
        .execute(
            "INSERT INTO media_files (
                media_item_id, scan_source_id, path, file_name, extension,
                file_size, modified_at, fingerprint, duration_seconds,
                width, height, video_codec, audio_codec, container_format, hdr_format,
                season_number, episode_number
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17
             )",
            params![
                media_item_id,
                scan_source_id,
                path_text,
                file_name,
                extension,
                file_size,
                modified_at,
                fingerprint,
                probe.duration_seconds,
                probe.width,
                probe.height,
                probe.video_codec,
                probe.audio_codec,
                probe.container_format,
                probe.hdr_format,
                parsed.season_number,
                parsed.episode_number,
            ],
        )
        .map_err(|error| format!("无法保存媒体文件: {error}"))?;

    Ok(UpsertOutcome::Added)
}

fn update_parsed_media(
    connection: &Connection,
    media_item_id: i64,
    parsed: &ParsedMediaName,
) -> Result<(), String> {
    connection
        .execute(
            "UPDATE media_items
             SET title = ?2, sort_title = ?2, year = ?3, media_type = ?4,
                 recognition_status = 'recognized',
                 season_number = NULL, episode_number = NULL, group_key = ?5,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1 AND recognition_status != 'manual'",
            params![
                media_item_id,
                parsed.title,
                parsed.year,
                parsed.media_type,
                parsed.group_key
            ],
        )
        .map_err(|error| format!("无法更新解析结果: {error}"))?;
    Ok(())
}

fn find_grouped_media_item(
    connection: &Connection,
    parsed: &ParsedMediaName,
) -> Result<Option<i64>, String> {
    if parsed.group_key.is_empty() {
        return Ok(None);
    }

    let result = match parsed.media_type {
        "series" | "animation" => connection.query_row(
            "SELECT id FROM media_items
             WHERE media_type = ?1 AND group_key = ?2
             ORDER BY (recognition_status = 'manual') DESC, id ASC
             LIMIT 1",
            params![parsed.media_type, parsed.group_key],
            |row| row.get(0),
        ),
        "movie" if parsed.year.is_some() => connection.query_row(
            "SELECT id FROM media_items
             WHERE media_type = 'movie' AND group_key = ?1 AND year = ?2
             ORDER BY (recognition_status = 'manual') DESC, id ASC
             LIMIT 1",
            params![parsed.group_key, parsed.year],
            |row| row.get(0),
        ),
        _ => return Ok(None),
    };

    result
        .optional()
        .map_err(|error| format!("无法查找同组影视条目: {error}"))
}

fn is_supported_video(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| VIDEO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn system_time_text(time: SystemTime) -> String {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

#[allow(clippy::too_many_arguments)]
fn finish_scan(
    connection: &Connection,
    history_id: i64,
    status: &str,
    files_found: u64,
    files_ignored: u64,
    files_added: u64,
    files_updated: u64,
    files_missing: u64,
    error_message: Option<&str>,
) -> Result<(), String> {
    connection
        .execute(
            "UPDATE scan_history
             SET finished_at = CURRENT_TIMESTAMP, status = ?2,
                 files_found = ?3, files_ignored = ?4, files_added = ?5,
                 files_updated = ?6, files_missing = ?7,
                 error_message = ?8
             WHERE id = ?1",
            params![
                history_id,
                status,
                files_found,
                files_ignored,
                files_added,
                files_updated,
                files_missing,
                error_message
            ],
        )
        .map_err(|error| format!("无法完成扫描记录: {error}"))?;
    Ok(())
}

fn is_blacklisted(connection: &Connection, path: &Path) -> Result<bool, String> {
    let path_text = path.to_string_lossy();
    connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM media_blacklist
                WHERE path = ?1 COLLATE NOCASE
            )",
            params![path_text.as_ref()],
            |row| row.get(0),
        )
        .map_err(|error| format!("无法检查文件黑名单: {error}"))
}

fn emit_progress(app: &AppHandle, progress: &ScanProgress) {
    let _ = app.emit("scan-progress", progress.clone());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blacklist_path_match_is_case_insensitive() {
        let connection = Connection::open_in_memory().expect("open test database");
        connection
            .execute_batch(
                "CREATE TABLE media_blacklist (
                    id INTEGER PRIMARY KEY,
                    path TEXT NOT NULL UNIQUE COLLATE NOCASE
                );
                INSERT INTO media_blacklist (path)
                VALUES ('A:\\Media\\Anime\\Episode01.mkv');",
            )
            .expect("create blacklist");

        assert!(
            is_blacklisted(&connection, Path::new(r"a:\media\anime\episode01.MKV"))
                .expect("query blacklist")
        );
        assert!(
            !is_blacklisted(&connection, Path::new(r"A:\Media\Anime\Episode02.mkv"))
                .expect("query blacklist")
        );
    }
}
