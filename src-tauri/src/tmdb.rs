use std::{fs, path::Path};

use reqwest::{Client, StatusCode};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::database::{lock_connection, DatabaseState};

const TMDB_API_ROOT: &str = "https://api.themoviedb.org/3";
const TMDB_IMAGE_ROOT: &str = "https://image.tmdb.org/t/p/w500";
const TMDB_TOKEN_KEY: &str = "tmdb_read_access_token";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TmdbStatus {
    configured: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveTmdbTokenRequest {
    token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchTmdbRequest {
    media_id: i64,
    query: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyTmdbRequest {
    media_id: i64,
    tmdb_id: i64,
    media_type: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TmdbCandidate {
    provider: &'static str,
    tmdb_id: i64,
    media_type: String,
    title: String,
    original_title: Option<String>,
    year: Option<i32>,
    overview: Option<String>,
    poster_url: Option<String>,
    vote_average: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyTmdbResult {
    title: String,
    poster_path: Option<String>,
    fields_applied: Vec<String>,
}

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<SearchItem>,
}

#[derive(Deserialize)]
struct SearchItem {
    id: i64,
    media_type: String,
    title: Option<String>,
    name: Option<String>,
    original_title: Option<String>,
    original_name: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    vote_average: Option<f64>,
    adult: Option<bool>,
}

#[derive(Deserialize, Serialize)]
struct TmdbDetails {
    id: i64,
    title: Option<String>,
    name: Option<String>,
    original_title: Option<String>,
    original_name: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    vote_average: Option<f64>,
    runtime: Option<i32>,
    episode_run_time: Option<Vec<i32>>,
}

#[tauri::command]
pub fn tmdb_status(state: State<'_, DatabaseState>) -> Result<TmdbStatus, String> {
    Ok(TmdbStatus {
        configured: load_token(&state)?.is_some(),
    })
}

#[tauri::command]
pub fn save_tmdb_token(
    request: SaveTmdbTokenRequest,
    state: State<'_, DatabaseState>,
) -> Result<TmdbStatus, String> {
    let token = request.token.trim();
    if token.is_empty() {
        return Err("TMDB Read Access Token 不能为空".to_string());
    }
    let connection = lock_connection(&state)?;
    connection
        .execute(
            "INSERT INTO app_settings (key, value)
             VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = CURRENT_TIMESTAMP",
            params![TMDB_TOKEN_KEY, token],
        )
        .map_err(|error| format!("无法保存 TMDB Token: {error}"))?;
    Ok(TmdbStatus { configured: true })
}

#[tauri::command]
pub async fn search_tmdb(
    request: SearchTmdbRequest,
    state: State<'_, DatabaseState>,
) -> Result<Vec<TmdbCandidate>, String> {
    let (token, default_title, default_year) = {
        let connection = lock_connection(&state)?;
        let token = query_token(&connection)?
            .ok_or_else(|| "请先在“在线刮削”页面配置 TMDB Read Access Token".to_string())?;
        let item = connection
            .query_row(
                "SELECT title, year FROM media_items WHERE id = ?1",
                [request.media_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i32>>(1)?)),
            )
            .optional()
            .map_err(|error| format!("无法读取待刮削条目: {error}"))?
            .ok_or_else(|| "影视条目不存在".to_string())?;
        (token, item.0, item.1)
    };
    let query = request
        .query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&default_title);
    let client = tmdb_client()?;
    let response = client
        .get(format!("{TMDB_API_ROOT}/search/multi"))
        .bearer_auth(token)
        .query(&[
            ("query", query),
            ("language", "zh-CN"),
            ("include_adult", "false"),
        ])
        .send()
        .await
        .map_err(|error| format!("无法连接 TMDB: {error}"))?;
    let response = checked_response(response).await?;
    let body = response
        .json::<SearchResponse>()
        .await
        .map_err(|error| format!("无法解析 TMDB 搜索结果: {error}"))?;

    let mut candidates = body
        .results
        .into_iter()
        .filter(|item| {
            (item.media_type == "movie" || item.media_type == "tv")
                && item.adult != Some(true)
                && (item.title.is_some() || item.name.is_some())
        })
        .map(|item| {
            let date = item
                .release_date
                .as_deref()
                .or(item.first_air_date.as_deref());
            let year = parse_year(date);
            TmdbCandidate {
                provider: "tmdb",
                tmdb_id: item.id,
                media_type: item.media_type,
                title: item.title.or(item.name).unwrap_or_default(),
                original_title: item.original_title.or(item.original_name),
                year,
                overview: clean_text(item.overview),
                poster_url: item
                    .poster_path
                    .map(|path| format!("{TMDB_IMAGE_ROOT}{path}")),
                vote_average: item.vote_average.filter(|value| *value > 0.0),
            }
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| {
        let year_distance = match (default_year, candidate.year) {
            (Some(expected), Some(actual)) => (expected - actual).unsigned_abs(),
            _ => 100,
        };
        (year_distance, candidate.poster_url.is_none())
    });
    candidates.truncate(12);
    log::info!(
        "TMDB search completed for media {} with {} candidates",
        request.media_id,
        candidates.len()
    );
    Ok(candidates)
}

#[tauri::command]
pub async fn apply_tmdb_metadata(
    app: AppHandle,
    request: ApplyTmdbRequest,
    state: State<'_, DatabaseState>,
) -> Result<ApplyTmdbResult, String> {
    if request.media_type != "movie" && request.media_type != "tv" {
        return Err("无效的 TMDB 媒体类型".to_string());
    }
    let token = load_token(&state)?.ok_or_else(|| "请先配置 TMDB Read Access Token".to_string())?;
    let client = tmdb_client()?;
    let response = client
        .get(format!(
            "{TMDB_API_ROOT}/{}/{}",
            request.media_type, request.tmdb_id
        ))
        .bearer_auth(&token)
        .query(&[("language", "zh-CN")])
        .send()
        .await
        .map_err(|error| format!("无法读取 TMDB 详情: {error}"))?;
    let response = checked_response(response).await?;
    let details = response
        .json::<TmdbDetails>()
        .await
        .map_err(|error| format!("无法解析 TMDB 详情: {error}"))?;
    let title = details
        .title
        .clone()
        .or_else(|| details.name.clone())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "TMDB 返回的标题为空".to_string())?;
    let original_title = details
        .original_title
        .clone()
        .or_else(|| details.original_name.clone());
    let date = details
        .release_date
        .as_deref()
        .or(details.first_air_date.as_deref());
    let year = parse_year(date);
    let runtime = details.runtime.or_else(|| {
        details
            .episode_run_time
            .as_ref()
            .and_then(|values| values.first().copied())
    });
    let overview = clean_text(details.overview.clone());
    let rating = details.vote_average.filter(|value| *value > 0.0);
    let metadata_json = serde_json::to_string(&details)
        .map_err(|error| format!("无法序列化 TMDB 资料: {error}"))?;
    let library_media_type = if request.media_type == "movie" {
        "movie"
    } else {
        "series"
    };

    let poster_bytes = if let Some(path) = details.poster_path.as_deref() {
        let response = client
            .get(format!("{TMDB_IMAGE_ROOT}{path}"))
            .send()
            .await
            .map_err(|error| format!("无法下载 TMDB 海报: {error}"))?;
        let response = checked_response(response).await?;
        Some((
            response
                .bytes()
                .await
                .map_err(|error| format!("无法读取 TMDB 海报: {error}"))?
                .to_vec(),
            Path::new(path)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("jpg")
                .to_ascii_lowercase(),
            format!("{TMDB_IMAGE_ROOT}{path}"),
        ))
    } else {
        None
    };

    let mut connection = lock_connection(&state)?;
    connection
        .execute(
            "UPDATE media_items
             SET title = ?2, original_title = ?3, sort_title = ?2,
                 year = COALESCE(?4, year), overview = COALESCE(?5, overview),
                 rating = COALESCE(?6, rating),
                 runtime_minutes = COALESCE(?7, runtime_minutes),
                 media_type = ?8, recognition_status = 'manual',
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![
                request.media_id,
                title,
                original_title,
                year,
                overview,
                rating,
                runtime,
                library_media_type
            ],
        )
        .map_err(|error| format!("无法保存 TMDB 资料: {error}"))?;
    connection
        .execute(
            "INSERT INTO external_metadata
                (media_item_id, provider_id, external_id, metadata_json)
             VALUES (?1, 'tmdb', ?2, ?3)
             ON CONFLICT(provider_id, external_id) DO UPDATE SET
                media_item_id = excluded.media_item_id,
                metadata_json = excluded.metadata_json,
                fetched_at = CURRENT_TIMESTAMP",
            params![
                request.media_id,
                format!("{}:{}", request.media_type, details.id),
                metadata_json
            ],
        )
        .map_err(|error| format!("无法保存 TMDB 来源记录: {error}"))?;

    let poster_path = poster_bytes
        .map(|(bytes, extension, source_url)| {
            cache_poster_bytes(
                &app,
                &mut connection,
                request.media_id,
                &bytes,
                &extension,
                &source_url,
            )
        })
        .transpose()?;
    let mut fields_applied = vec!["标题".to_string()];
    if original_title.is_some() {
        fields_applied.push("原始标题".to_string());
    }
    if year.is_some() {
        fields_applied.push("年份".to_string());
    }
    if overview.is_some() {
        fields_applied.push("中文简介".to_string());
    }
    if rating.is_some() {
        fields_applied.push("评分".to_string());
    }
    if runtime.is_some() {
        fields_applied.push("片长".to_string());
    }
    if poster_path.is_some() {
        fields_applied.push("海报".to_string());
    }
    log::info!(
        "TMDB metadata applied for media {} from {}:{}",
        request.media_id,
        request.media_type,
        request.tmdb_id
    );
    Ok(ApplyTmdbResult {
        title,
        poster_path,
        fields_applied,
    })
}

fn load_token(state: &State<'_, DatabaseState>) -> Result<Option<String>, String> {
    let connection = lock_connection(state)?;
    query_token(&connection)
}

fn query_token(connection: &rusqlite::Connection) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [TMDB_TOKEN_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("无法读取 TMDB 设置: {error}"))
}

fn tmdb_client() -> Result<Client, String> {
    Client::builder()
        .user_agent("MediaManager/0.1")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|error| format!("无法初始化网络客户端: {error}"))
}

async fn checked_response(response: reqwest::Response) -> Result<reqwest::Response, String> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let message = response.text().await.unwrap_or_default();
    match status {
        StatusCode::UNAUTHORIZED => Err("TMDB Token 无效或已失效".to_string()),
        StatusCode::TOO_MANY_REQUESTS => Err("TMDB 请求过于频繁，请稍后再试".to_string()),
        _ => Err(format!("TMDB 请求失败（{status}）：{message}")),
    }
}

fn cache_poster_bytes(
    app: &AppHandle,
    connection: &mut rusqlite::Connection,
    media_id: i64,
    bytes: &[u8],
    extension: &str,
    source_url: &str,
) -> Result<String, String> {
    let poster_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| format!("无法确定应用数据目录: {error}"))?
        .join("cache")
        .join("posters");
    fs::create_dir_all(&poster_dir).map_err(|error| format!("无法创建海报缓存目录: {error}"))?;
    let destination = poster_dir.join(format!("{media_id}.{extension}"));
    fs::write(&destination, bytes).map_err(|error| format!("无法缓存 TMDB 海报: {error}"))?;
    let destination_text = destination.to_string_lossy().into_owned();
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始 TMDB 海报更新: {error}"))?;
    transaction
        .execute(
            "UPDATE artwork SET is_primary = 0
             WHERE media_item_id = ?1 AND artwork_type = 'poster'",
            [media_id],
        )
        .map_err(|error| format!("无法更新旧海报: {error}"))?;
    transaction
        .execute(
            "INSERT INTO artwork
                (media_item_id, artwork_type, local_path, source_url, is_primary)
             VALUES (?1, 'poster', ?2, ?3, 1)",
            params![media_id, destination_text, source_url],
        )
        .map_err(|error| format!("无法保存 TMDB 海报记录: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("无法提交 TMDB 海报: {error}"))?;
    Ok(destination_text)
}

fn parse_year(date: Option<&str>) -> Option<i32> {
    date.and_then(|value| value.get(..4))
        .and_then(|value| value.parse().ok())
}

fn clean_text(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tmdb_year() {
        assert_eq!(parse_year(Some("2025-04-06")), Some(2025));
        assert_eq!(parse_year(Some("")), None);
        assert_eq!(parse_year(None), None);
    }
}
