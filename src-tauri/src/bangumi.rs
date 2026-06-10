use std::{fs, path::Path};

use reqwest::{Client, StatusCode};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::database::{lock_connection, DatabaseState};

const BANGUMI_API: &str = "https://api.bgm.tv/v0";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchBangumiRequest {
    query: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyBangumiRequest {
    media_id: i64,
    bangumi_id: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangumiCandidate {
    provider: &'static str,
    bangumi_id: i64,
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
pub struct ApplyBangumiResult {
    title: String,
    poster_path: Option<String>,
    fields_applied: Vec<String>,
}

#[derive(Deserialize)]
struct SearchResponse {
    data: Vec<BangumiSubject>,
}

#[derive(Clone, Deserialize, Serialize)]
struct BangumiSubject {
    id: i64,
    name: String,
    name_cn: String,
    summary: String,
    date: Option<String>,
    images: Option<BangumiImages>,
    rating: Option<BangumiRating>,
}

#[derive(Clone, Deserialize, Serialize)]
struct BangumiImages {
    large: Option<String>,
    common: Option<String>,
    medium: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
struct BangumiRating {
    score: Option<f64>,
}

#[tauri::command]
pub async fn search_bangumi(
    request: SearchBangumiRequest,
) -> Result<Vec<BangumiCandidate>, String> {
    let query = request.query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let response = bangumi_client()?
        .post(format!("{BANGUMI_API}/search/subjects"))
        .query(&[("limit", "12"), ("offset", "0")])
        .json(&serde_json::json!({
            "keyword": query,
            "filter": { "type": [2] }
        }))
        .send()
        .await
        .map_err(|error| format!("无法连接 Bangumi: {error}"))?;
    let body = checked_response(response)
        .await?
        .json::<SearchResponse>()
        .await
        .map_err(|error| format!("无法解析 Bangumi 搜索结果: {error}"))?;

    let mut candidates = body
        .data
        .into_iter()
        .map(candidate_from_subject)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| {
        let expected = normalize_title(query);
        let title = normalize_title(&candidate.title);
        let original = candidate
            .original_title
            .as_deref()
            .map(normalize_title)
            .unwrap_or_default();
        if title == expected {
            0
        } else if original == expected {
            1
        } else if title.contains(&expected) || expected.contains(&title) {
            2
        } else {
            3
        }
    });
    Ok(candidates)
}

#[tauri::command]
pub async fn apply_bangumi_metadata(
    app: AppHandle,
    request: ApplyBangumiRequest,
    state: State<'_, DatabaseState>,
) -> Result<ApplyBangumiResult, String> {
    let response = bangumi_client()?
        .get(format!("{BANGUMI_API}/subjects/{}", request.bangumi_id))
        .send()
        .await
        .map_err(|error| format!("无法读取 Bangumi 详情: {error}"))?;
    let subject = checked_response(response)
        .await?
        .json::<BangumiSubject>()
        .await
        .map_err(|error| format!("无法解析 Bangumi 详情: {error}"))?;

    let title = preferred_title(&subject);
    let original_title = non_empty(&subject.name);
    let overview = non_empty(&subject.summary);
    let year = subject.date.as_deref().and_then(parse_year);
    let rating = subject.rating.as_ref().and_then(|value| value.score);
    let poster_url = poster_url(&subject);
    let metadata_json = serde_json::to_string(&subject)
        .map_err(|error| format!("无法序列化 Bangumi 资料: {error}"))?;
    let poster_bytes = if let Some(url) = poster_url.as_deref() {
        let response = bangumi_client()?
            .get(url)
            .send()
            .await
            .map_err(|error| format!("无法下载 Bangumi 封面: {error}"))?;
        let response = checked_response(response).await?;
        Some((
            response
                .bytes()
                .await
                .map_err(|error| format!("无法读取 Bangumi 封面: {error}"))?
                .to_vec(),
            image_extension(url),
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
                 rating = COALESCE(?6, rating), media_type = 'animation',
                 recognition_status = 'manual', updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![
                request.media_id,
                title,
                original_title,
                year,
                overview,
                rating
            ],
        )
        .map_err(|error| format!("无法保存 Bangumi 资料: {error}"))?;
    connection
        .execute(
            "INSERT INTO external_metadata
                (media_item_id, provider_id, external_id, metadata_json)
             VALUES (?1, 'bangumi', ?2, ?3)
             ON CONFLICT(provider_id, external_id) DO UPDATE SET
                media_item_id = excluded.media_item_id,
                metadata_json = excluded.metadata_json,
                fetched_at = CURRENT_TIMESTAMP",
            params![request.media_id, subject.id.to_string(), metadata_json],
        )
        .map_err(|error| format!("无法保存 Bangumi 来源记录: {error}"))?;
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

    let mut fields_applied = vec!["中文标题".to_string(), "动画分类".to_string()];
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
    if poster_path.is_some() {
        fields_applied.push("封面".to_string());
    }
    Ok(ApplyBangumiResult {
        title,
        poster_path,
        fields_applied,
    })
}

fn candidate_from_subject(subject: BangumiSubject) -> BangumiCandidate {
    BangumiCandidate {
        provider: "bangumi",
        bangumi_id: subject.id,
        media_type: "anime",
        title: preferred_title(&subject),
        original_title: non_empty(&subject.name),
        year: subject.date.as_deref().and_then(parse_year),
        overview: non_empty(&subject.summary),
        poster_url: poster_url(&subject),
        vote_average: subject.rating.and_then(|value| value.score),
    }
}

fn preferred_title(subject: &BangumiSubject) -> String {
    non_empty(&subject.name_cn)
        .or_else(|| non_empty(&subject.name))
        .unwrap_or_else(|| "未命名动画".to_string())
}

fn poster_url(subject: &BangumiSubject) -> Option<String> {
    subject.images.as_ref().and_then(|images| {
        images
            .large
            .clone()
            .or_else(|| images.common.clone())
            .or_else(|| images.medium.clone())
    })
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn parse_year(value: &str) -> Option<i32> {
    value.get(..4)?.parse().ok()
}

fn normalize_title(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn image_extension(url: &str) -> String {
    Path::new(url)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("jpg")
        .split('?')
        .next()
        .unwrap_or("jpg")
        .to_ascii_lowercase()
}

fn bangumi_client() -> Result<Client, String> {
    Client::builder()
        .user_agent("MediaManager/0.1 (local desktop media manager)")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|error| format!("无法初始化 Bangumi 网络客户端: {error}"))
}

async fn checked_response(response: reqwest::Response) -> Result<reqwest::Response, String> {
    if response.status().is_success() {
        return Ok(response);
    }
    match response.status() {
        StatusCode::TOO_MANY_REQUESTS => Err("Bangumi 请求过于频繁，请稍后再试".to_string()),
        StatusCode::NOT_FOUND => Err("Bangumi 中没有找到该动画".to_string()),
        status => Err(format!("Bangumi 请求失败：{status}")),
    }
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
    fs::create_dir_all(&poster_dir).map_err(|error| format!("无法创建封面缓存目录: {error}"))?;
    let destination = poster_dir.join(format!("{media_id}.{extension}"));
    fs::write(&destination, bytes).map_err(|error| format!("无法缓存 Bangumi 封面: {error}"))?;
    let destination_text = destination.to_string_lossy().into_owned();
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始 Bangumi 封面更新: {error}"))?;
    transaction
        .execute(
            "UPDATE artwork SET is_primary = 0
             WHERE media_item_id = ?1 AND artwork_type = 'poster'",
            [media_id],
        )
        .map_err(|error| format!("无法更新旧封面: {error}"))?;
    transaction
        .execute(
            "INSERT INTO artwork
                (media_item_id, artwork_type, local_path, source_url, is_primary)
             VALUES (?1, 'poster', ?2, ?3, 1)",
            params![media_id, destination_text, source_url],
        )
        .map_err(|error| format!("无法保存 Bangumi 封面记录: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("无法提交 Bangumi 封面: {error}"))?;
    Ok(destination_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_chinese_title_and_parses_year() {
        let subject = BangumiSubject {
            id: 1,
            name: "Sousou no Frieren".to_string(),
            name_cn: "葬送的芙莉莲".to_string(),
            summary: String::new(),
            date: Some("2023-09-29".to_string()),
            images: None,
            rating: None,
        };
        assert_eq!(preferred_title(&subject), "葬送的芙莉莲");
        assert_eq!(subject.date.as_deref().and_then(parse_year), Some(2023));
    }
}
