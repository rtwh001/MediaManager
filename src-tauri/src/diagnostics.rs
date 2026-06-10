use std::{fs, time::SystemTime};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::database::{lock_connection, DatabaseState};
use crate::probe;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsReport {
    app_version: String,
    database_path: String,
    database_size_bytes: u64,
    log_directory: String,
    schema_version: i64,
    media_count: i64,
    file_count: i64,
    missing_file_count: i64,
    scan_source_count: i64,
    failed_scan_count: i64,
    ffprobe_available: bool,
    ffprobe_version: Option<String>,
}

#[tauri::command]
pub fn diagnostics_report(
    app: AppHandle,
    state: State<'_, DatabaseState>,
) -> Result<DiagnosticsReport, String> {
    let connection = lock_connection(&state)?;
    let schema_version = scalar(
        &connection,
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
    )?;
    let media_count = scalar(&connection, "SELECT COUNT(*) FROM media_items")?;
    let file_count = scalar(&connection, "SELECT COUNT(*) FROM media_files")?;
    let missing_file_count = scalar(
        &connection,
        "SELECT COUNT(*) FROM media_files WHERE is_missing = 1",
    )?;
    let scan_source_count = scalar(&connection, "SELECT COUNT(*) FROM scan_sources")?;
    let failed_scan_count = scalar(
        &connection,
        "SELECT COUNT(*) FROM scan_history WHERE status = 'failed'",
    )?;
    drop(connection);

    let ffprobe_version = probe::ffprobe_version();
    let log_directory = app
        .path()
        .app_log_dir()
        .map_err(|error| format!("无法确定日志目录: {error}"))?;
    let database_size_bytes = fs::metadata(state.path())
        .map(|metadata| metadata.len())
        .unwrap_or(0);

    Ok(DiagnosticsReport {
        app_version: app.package_info().version.to_string(),
        database_path: state.path().to_string_lossy().into_owned(),
        database_size_bytes,
        log_directory: log_directory.to_string_lossy().into_owned(),
        schema_version,
        media_count,
        file_count,
        missing_file_count,
        scan_source_count,
        failed_scan_count,
        ffprobe_available: ffprobe_version.is_some(),
        ffprobe_version,
    })
}

#[tauri::command]
pub fn read_recent_logs(app: AppHandle) -> Result<String, String> {
    let log_directory = app
        .path()
        .app_log_dir()
        .map_err(|error| format!("无法确定日志目录: {error}"))?;
    if !log_directory.is_dir() {
        return Ok("日志目录尚未创建。".to_string());
    }

    let mut log_files = fs::read_dir(&log_directory)
        .map_err(|error| format!("无法读取日志目录: {error}"))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case("log"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    log_files.sort_by_key(|entry| {
        entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH)
    });

    let Some(latest) = log_files.last() else {
        return Ok("还没有可读取的日志文件。".to_string());
    };
    let contents =
        fs::read_to_string(latest.path()).map_err(|error| format!("无法读取日志文件: {error}"))?;
    let mut lines = contents.lines().rev().take(500).collect::<Vec<_>>();
    lines.reverse();
    Ok(lines.join("\n"))
}

fn scalar(connection: &rusqlite::Connection, sql: &str) -> Result<i64, String> {
    connection
        .query_row(sql, [], |row| row.get(0))
        .map_err(|error| format!("无法生成诊断信息: {error}"))
}
