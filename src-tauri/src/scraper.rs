use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use quick_xml::de::from_str;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::database::{lock_connection, DatabaseState};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct LocalNfo {
    title: Option<String>,
    originaltitle: Option<String>,
    sorttitle: Option<String>,
    year: Option<i32>,
    plot: Option<String>,
    outline: Option<String>,
    rating: Option<f64>,
    runtime: Option<i32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeResult {
    provider: &'static str,
    nfo_path: Option<String>,
    poster_path: Option<String>,
    fields_applied: Vec<String>,
    message: String,
}

#[tauri::command]
pub fn scrape_local_metadata(
    app: AppHandle,
    media_id: i64,
    state: State<'_, DatabaseState>,
) -> Result<ScrapeResult, String> {
    let mut connection = lock_connection(&state)?;
    let media_type = connection
        .query_row(
            "SELECT media_type FROM media_items WHERE id = ?1",
            [media_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("无法读取影视条目: {error}"))?
        .ok_or_else(|| "影视条目不存在".to_string())?;
    let file_paths = {
        let mut statement = connection
            .prepare(
                "SELECT path FROM media_files
                 WHERE media_item_id = ?1 AND is_missing = 0
                 ORDER BY season_number, episode_number, id",
            )
            .map_err(|error| format!("无法读取媒体文件: {error}"))?;
        let paths = statement
            .query_map([media_id], |row| row.get::<_, String>(0))
            .map_err(|error| format!("无法查询媒体文件: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("无法解析媒体文件: {error}"))?;
        paths
    };
    if file_paths.is_empty() {
        return Err("没有可用于刮削的本地媒体文件".to_string());
    }

    let candidates = candidate_directories(&file_paths, &media_type);
    let nfo_path = find_nfo(&file_paths, &candidates, &media_type);
    let poster_source = find_poster(&file_paths, &candidates);
    let parsed_nfo = nfo_path.as_deref().map(read_nfo).transpose()?.flatten();
    let nfo = parsed_nfo.clone().unwrap_or_default();
    let mut fields_applied = Vec::new();

    if parsed_nfo.is_some() {
        let overview = clean_text(nfo.plot.clone()).or_else(|| clean_text(nfo.outline.clone()));
        let title = clean_text(nfo.title.clone());
        let original_title = clean_text(nfo.originaltitle.clone());
        let sort_title = clean_text(nfo.sorttitle.clone());
        if title.is_some() {
            fields_applied.push("标题".to_string());
        }
        if original_title.is_some() {
            fields_applied.push("原始标题".to_string());
        }
        if nfo.year.is_some() {
            fields_applied.push("年份".to_string());
        }
        if overview.is_some() {
            fields_applied.push("简介".to_string());
        }
        if nfo.rating.is_some() {
            fields_applied.push("评分".to_string());
        }
        if nfo.runtime.is_some() {
            fields_applied.push("片长".to_string());
        }

        connection
            .execute(
                "UPDATE media_items
                 SET title = COALESCE(?2, title),
                     original_title = COALESCE(?3, original_title),
                     sort_title = COALESCE(?4, ?2, sort_title),
                     year = COALESCE(?5, year),
                     overview = COALESCE(?6, overview),
                     rating = COALESCE(?7, rating),
                     runtime_minutes = COALESCE(?8, runtime_minutes),
                     recognition_status = 'recognized',
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?1",
                params![
                    media_id,
                    title,
                    original_title,
                    sort_title,
                    nfo.year,
                    overview,
                    nfo.rating,
                    nfo.runtime
                ],
            )
            .map_err(|error| format!("无法保存本地 NFO 资料: {error}"))?;

        let nfo_text =
            serde_json::to_string(&nfo).map_err(|error| format!("无法序列化 NFO 资料: {error}"))?;
        let external_id = nfo_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        connection
            .execute(
                "INSERT INTO external_metadata
                    (media_item_id, provider_id, external_id, metadata_json)
                 VALUES (?1, 'local-nfo', ?2, ?3)
                 ON CONFLICT(provider_id, external_id) DO UPDATE SET
                    media_item_id = excluded.media_item_id,
                    metadata_json = excluded.metadata_json,
                    fetched_at = CURRENT_TIMESTAMP",
                params![media_id, external_id, nfo_text],
            )
            .map_err(|error| format!("无法保存刮削来源记录: {error}"))?;
    }

    let poster_path = poster_source
        .as_deref()
        .map(|source| cache_poster(&app, &mut connection, media_id, source))
        .transpose()?;
    if poster_path.is_some() {
        fields_applied.push("海报".to_string());
    }

    if parsed_nfo.is_none() && poster_path.is_none() {
        return Ok(ScrapeResult {
            provider: "local-nfo",
            nfo_path: nfo_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            poster_path: None,
            fields_applied,
            message: if nfo_path.is_some() {
                "发现的 .nfo 不是 XML 影视资料，已跳过。".to_string()
            } else {
                "未找到 NFO 或同目录海报。".to_string()
            },
        });
    }

    log::info!(
        "local scrape completed for media {}: nfo={}, poster={}, fields={}",
        media_id,
        parsed_nfo.is_some(),
        poster_path.is_some(),
        fields_applied.join(",")
    );
    Ok(ScrapeResult {
        provider: "local-nfo",
        nfo_path: nfo_path.map(|path| path.to_string_lossy().into_owned()),
        poster_path,
        message: format!("本地刮削完成，更新 {} 项资料。", fields_applied.len()),
        fields_applied,
    })
}

fn candidate_directories(file_paths: &[String], media_type: &str) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut directories = Vec::new();
    for path in file_paths {
        let Some(parent) = Path::new(path).parent() else {
            continue;
        };
        let candidate = if media_type == "series" || media_type == "animation" {
            if looks_like_season_folder(parent) {
                parent.parent().unwrap_or(parent)
            } else {
                parent
            }
        } else {
            parent
        };
        if seen.insert(candidate.to_path_buf()) {
            directories.push(candidate.to_path_buf());
        }
    }
    directories
}

fn looks_like_season_folder(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    name.starts_with("season ")
        || (name.starts_with('s') && name[1..].chars().all(|value| value.is_ascii_digit()))
        || (name.starts_with('第') && name.ends_with('季'))
}

fn find_nfo(file_paths: &[String], directories: &[PathBuf], media_type: &str) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for directory in directories {
        if media_type == "series" || media_type == "animation" {
            candidates.push(directory.join("tvshow.nfo"));
            candidates.push(directory.join("show.nfo"));
        } else {
            candidates.push(directory.join("movie.nfo"));
        }
        if let Some(folder_name) = directory.file_name() {
            candidates.push(directory.join(Path::new(folder_name).with_extension("nfo")));
        }
    }
    if let Some(first_file) = file_paths.first() {
        candidates.push(Path::new(first_file).with_extension("nfo"));
    }
    first_existing_case_insensitive(candidates)
}

fn find_poster(file_paths: &[String], directories: &[PathBuf]) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for directory in directories {
        for stem in ["poster", "folder", "cover", "movie", "tvshow"] {
            for extension in ["jpg", "jpeg", "png", "webp"] {
                candidates.push(directory.join(format!("{stem}.{extension}")));
            }
        }
    }
    if let Some(first_file) = file_paths.first() {
        let path = Path::new(first_file);
        if let (Some(parent), Some(stem)) = (path.parent(), path.file_stem()) {
            for extension in ["jpg", "jpeg", "png", "webp"] {
                candidates
                    .push(parent.join(format!("{}-poster.{extension}", stem.to_string_lossy())));
                candidates.push(parent.join(format!("{}.{extension}", stem.to_string_lossy())));
            }
        }
    }
    first_existing_case_insensitive(candidates)
}

fn first_existing_case_insensitive(candidates: Vec<PathBuf>) -> Option<PathBuf> {
    for candidate in candidates {
        if candidate.is_file() {
            return Some(candidate);
        }
        let Some(parent) = candidate.parent() else {
            continue;
        };
        let Some(expected) = candidate.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let Ok(entries) = fs::read_dir(parent) else {
            continue;
        };
        let found = entries.filter_map(Result::ok).find(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|value| value.eq_ignore_ascii_case(expected))
                .unwrap_or(false)
        });
        if let Some(entry) = found {
            return Some(entry.path());
        }
    }
    None
}

fn read_nfo(path: &Path) -> Result<Option<LocalNfo>, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("无法读取 NFO {}: {error}", path.display()))?;
    let text = decode_text(&bytes);
    if !text.trim_start().starts_with('<') {
        log::warn!("ignored non-XML NFO: {}", path.display());
        return Ok(None);
    }
    match from_str(&text) {
        Ok(nfo) => Ok(Some(nfo)),
        Err(error) => {
            log::warn!("ignored invalid XML NFO {}: {error}", path.display());
            Ok(None)
        }
    }
}

fn decode_text(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&bytes[3..]).into_owned()
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        let values = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        String::from_utf16_lossy(&values)
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        let values = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        String::from_utf16_lossy(&values)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

fn cache_poster(
    app: &AppHandle,
    connection: &mut rusqlite::Connection,
    media_id: i64,
    source: &Path,
) -> Result<String, String> {
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "本地海报没有扩展名".to_string())?;
    let poster_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| format!("无法确定应用数据目录: {error}"))?
        .join("cache")
        .join("posters");
    fs::create_dir_all(&poster_dir).map_err(|error| format!("无法创建海报缓存目录: {error}"))?;
    let destination = poster_dir.join(format!("{media_id}.{extension}"));
    fs::copy(source, &destination).map_err(|error| format!("无法缓存本地海报: {error}"))?;
    let destination_text = destination.to_string_lossy().into_owned();

    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始本地海报更新: {error}"))?;
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
                (media_item_id, artwork_type, local_path, is_primary)
             VALUES (?1, 'poster', ?2, 1)",
            params![media_id, destination_text],
        )
        .map_err(|error| format!("无法保存本地海报: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("无法提交本地海报更新: {error}"))?;
    Ok(destination_text)
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
    fn parses_movie_nfo() {
        let nfo: LocalNfo = from_str(
            r#"<movie>
                <title>流浪地球 2</title>
                <originaltitle>The Wandering Earth II</originaltitle>
                <year>2023</year>
                <plot>人类建造行星发动机。</plot>
                <rating>8.3</rating>
                <runtime>173</runtime>
            </movie>"#,
        )
        .expect("movie nfo should parse");
        assert_eq!(nfo.title.as_deref(), Some("流浪地球 2"));
        assert_eq!(nfo.year, Some(2023));
        assert_eq!(nfo.runtime, Some(173));
    }

    #[test]
    fn decodes_utf16_nfo() {
        let source = "<movie><title>测试影片</title></movie>";
        let mut bytes = vec![0xFF, 0xFE];
        for value in source.encode_utf16() {
            bytes.extend(value.to_le_bytes());
        }
        assert_eq!(decode_text(&bytes), source);
    }

    #[test]
    fn ignores_plain_text_release_nfo() {
        let path = std::env::temp_dir().join(format!(
            "media-manager-release-notes-{}.nfo",
            std::process::id()
        ));
        fs::write(&path, "PublicHD - High-Definition Bittorrent Community")
            .expect("plain nfo should be written");
        let parsed = read_nfo(&path).expect("plain nfo should not fail");
        assert!(parsed.is_none());
        let _ = fs::remove_file(path);
    }
}
