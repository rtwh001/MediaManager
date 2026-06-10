use std::{fs, path::Path};

use regex::Regex;
use reqwest::{Client, StatusCode};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::database::{lock_connection, DatabaseState};

const ANILIST_API: &str = "https://graphql.anilist.co";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchAniListRequest {
    media_id: i64,
    query: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyAniListRequest {
    media_id: i64,
    anilist_id: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AniListCandidate {
    provider: &'static str,
    anilist_id: i64,
    media_type: &'static str,
    title: String,
    original_title: Option<String>,
    year: Option<i32>,
    overview: Option<String>,
    poster_url: Option<String>,
    vote_average: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyAniListResult {
    title: String,
    poster_path: Option<String>,
    fields_applied: Vec<String>,
}

#[derive(Deserialize)]
struct GraphQlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Deserialize)]
struct GraphQlError {
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SearchData {
    page: AniListPage,
}

#[derive(Deserialize)]
struct AniListPage {
    media: Vec<AniListMedia>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AniListMedia {
    id: i64,
    title: AniListTitle,
    description: Option<String>,
    start_date: AniListDate,
    cover_image: AniListCover,
    average_score: Option<f64>,
    duration: Option<i32>,
}

#[derive(Clone, Deserialize, Serialize)]
struct AniListTitle {
    romaji: Option<String>,
    english: Option<String>,
    native: Option<String>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct AniListDate {
    year: Option<i32>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct AniListCover {
    #[serde(rename = "extraLarge")]
    extra_large: Option<String>,
    large: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DetailsData {
    media: AniListMedia,
}

#[tauri::command]
pub async fn search_anilist(
    request: SearchAniListRequest,
    state: State<'_, DatabaseState>,
) -> Result<Vec<AniListCandidate>, String> {
    let default_title = {
        let connection = lock_connection(&state)?;
        connection
            .query_row(
                "SELECT title FROM media_items WHERE id = ?1",
                [request.media_id],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| format!("无法读取动画条目: {error}"))?
    };
    let query_text = request
        .query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&default_title);
    let query = r#"
        query ($search: String!) {
          Page(page: 1, perPage: 12) {
            media(search: $search, type: ANIME, sort: SEARCH_MATCH) {
              id
              title { romaji english native }
              description(asHtml: false)
              startDate { year }
              coverImage { extraLarge large }
              averageScore
              duration
            }
          }
        }
    "#;
    let response = anilist_client()?
        .post(ANILIST_API)
        .json(&serde_json::json!({
            "query": query,
            "variables": { "search": query_text }
        }))
        .send()
        .await
        .map_err(|error| format!("无法连接 AniList: {error}"))?;
    let body = parse_graphql::<SearchData>(response).await?;
    let candidates = body
        .page
        .media
        .into_iter()
        .map(|media| {
            let title = preferred_title(&media.title);
            AniListCandidate {
                provider: "anilist",
                anilist_id: media.id,
                media_type: "anime",
                title,
                original_title: media.title.native.or(media.title.romaji),
                year: media.start_date.year,
                overview: media.description.map(|value| clean_description(&value)),
                poster_url: media.cover_image.extra_large.or(media.cover_image.large),
                vote_average: media.average_score.map(|value| value / 10.0),
            }
        })
        .collect::<Vec<_>>();
    log::info!(
        "AniList search completed for media {} with {} candidates",
        request.media_id,
        candidates.len()
    );
    Ok(candidates)
}

#[tauri::command]
pub async fn apply_anilist_metadata(
    app: AppHandle,
    request: ApplyAniListRequest,
    state: State<'_, DatabaseState>,
) -> Result<ApplyAniListResult, String> {
    let query = r#"
        query ($id: Int!) {
          Media(id: $id, type: ANIME) {
            id
            title { romaji english native }
            description(asHtml: false)
            startDate { year }
            coverImage { extraLarge large }
            averageScore
            duration
          }
        }
    "#;
    let response = anilist_client()?
        .post(ANILIST_API)
        .json(&serde_json::json!({
            "query": query,
            "variables": { "id": request.anilist_id }
        }))
        .send()
        .await
        .map_err(|error| format!("无法读取 AniList 详情: {error}"))?;
    let media = parse_graphql::<DetailsData>(response).await?.media;
    let title = preferred_title(&media.title);
    let original_title = media.title.native.clone().or(media.title.romaji.clone());
    let overview = media.description.as_deref().map(clean_description);
    let rating = media.average_score.map(|value| value / 10.0);
    let metadata_json = serde_json::to_string(&media)
        .map_err(|error| format!("无法序列化 AniList 资料: {error}"))?;
    let poster_url = media.cover_image.extra_large.or(media.cover_image.large);
    let poster_bytes = if let Some(url) = poster_url.as_deref() {
        let response = anilist_client()?
            .get(url)
            .send()
            .await
            .map_err(|error| format!("无法下载 AniList 海报: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("AniList 海报下载失败：{}", response.status()));
        }
        Some((
            response
                .bytes()
                .await
                .map_err(|error| format!("无法读取 AniList 海报: {error}"))?
                .to_vec(),
            Path::new(url)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("jpg")
                .split('?')
                .next()
                .unwrap_or("jpg")
                .to_string(),
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
                 media_type = 'animation', recognition_status = 'manual',
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![
                request.media_id,
                title,
                original_title,
                media.start_date.year,
                overview,
                rating,
                media.duration
            ],
        )
        .map_err(|error| format!("无法保存 AniList 资料: {error}"))?;
    connection
        .execute(
            "INSERT INTO external_metadata
                (media_item_id, provider_id, external_id, metadata_json)
             VALUES (?1, 'anilist', ?2, ?3)
             ON CONFLICT(provider_id, external_id) DO UPDATE SET
                media_item_id = excluded.media_item_id,
                metadata_json = excluded.metadata_json,
                fetched_at = CURRENT_TIMESTAMP",
            params![request.media_id, media.id.to_string(), metadata_json],
        )
        .map_err(|error| format!("无法保存 AniList 来源记录: {error}"))?;
    let poster_path = poster_bytes
        .map(|(bytes, extension)| {
            cache_poster(
                &app,
                &mut connection,
                request.media_id,
                &bytes,
                &extension,
                poster_url.as_deref().unwrap_or_default(),
            )
        })
        .transpose()?;

    let mut fields_applied = vec!["标题".to_string(), "动画分类".to_string()];
    if original_title.is_some() {
        fields_applied.push("原始标题".to_string());
    }
    if media.start_date.year.is_some() {
        fields_applied.push("年份".to_string());
    }
    if overview.is_some() {
        fields_applied.push("简介".to_string());
    }
    if rating.is_some() {
        fields_applied.push("评分".to_string());
    }
    if media.duration.is_some() {
        fields_applied.push("单集时长".to_string());
    }
    if poster_path.is_some() {
        fields_applied.push("海报".to_string());
    }
    Ok(ApplyAniListResult {
        title,
        poster_path,
        fields_applied,
    })
}

fn preferred_title(title: &AniListTitle) -> String {
    title
        .english
        .as_ref()
        .or(title.romaji.as_ref())
        .or(title.native.as_ref())
        .cloned()
        .unwrap_or_else(|| "未命名动画".to_string())
}

fn clean_description(value: &str) -> String {
    let tags = Regex::new(r"<[^>]+>").expect("valid HTML tag regex");
    let breaks = value
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n");
    tags.replace_all(&breaks, "").trim().to_string()
}

fn anilist_client() -> Result<Client, String> {
    Client::builder()
        .user_agent("MediaManager/0.1")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|error| format!("无法初始化 AniList 网络客户端: {error}"))
}

async fn parse_graphql<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
) -> Result<T, String> {
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return Err("AniList 请求过于频繁，请稍后再试".to_string());
    }
    if !response.status().is_success() {
        return Err(format!("AniList 请求失败：{}", response.status()));
    }
    let body = response
        .json::<GraphQlResponse<T>>()
        .await
        .map_err(|error| format!("无法解析 AniList 响应: {error}"))?;
    if let Some(errors) = body.errors {
        return Err(errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; "));
    }
    body.data.ok_or_else(|| "AniList 没有返回资料".to_string())
}

fn cache_poster(
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
    fs::write(&destination, bytes).map_err(|error| format!("无法缓存 AniList 海报: {error}"))?;
    let destination_text = destination.to_string_lossy().into_owned();
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始 AniList 海报更新: {error}"))?;
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
        .map_err(|error| format!("无法保存 AniList 海报记录: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("无法提交 AniList 海报: {error}"))?;
    Ok(destination_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_description_markup() {
        assert_eq!(
            clean_description("First line<br><b>Second line</b>"),
            "First line\nSecond line"
        );
    }
}
