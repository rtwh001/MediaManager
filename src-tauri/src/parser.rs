use std::path::Path;

use regex::Regex;

#[derive(Debug, Clone)]
pub struct ParsedMediaName {
    pub title: String,
    pub group_key: String,
    pub year: Option<i32>,
    pub media_type: &'static str,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
}

pub fn parse_media_name(path: &Path, source_root: &Path) -> ParsedMediaName {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("未命名影片");
    let bracket_animation = parse_bracket_animation(stem);
    let normalized = normalize(stem);

    let mut season_episode = bracket_animation
        .as_ref()
        .map(|(_, episode)| (1, *episode))
        .or_else(|| parse_season_episode(&normalized));
    let year = parse_year(&normalized);
    let is_animation = bracket_animation.is_some() || path_looks_like_animation(path, source_root);
    if season_episode.is_none() && is_animation {
        season_episode = parse_loose_animation_episode(&normalized);
    }

    let parsed_title = clean_title(&normalized, year, season_episode);
    let parent_title = series_parent_title(path, source_root);
    let bracket_title = bracket_animation.map(|(title, _)| title);
    let should_use_parent = bracket_title.is_none()
        && (is_animation
            || (season_episode.is_some()
                && (parsed_title
                    .chars()
                    .all(|character| character.is_ascii_digit())
                    || parsed_title.len() < 2
                    || looks_like_release_group_only(&parsed_title))));
    let title = if let Some(title) = bracket_title {
        title
    } else if should_use_parent {
        parent_title.unwrap_or(parsed_title)
    } else {
        parsed_title
    };
    let media_type = if is_animation {
        "animation"
    } else if season_episode.is_some() {
        "series"
    } else {
        "movie"
    };

    ParsedMediaName {
        group_key: normalized_group_key(&title),
        title,
        year,
        media_type,
        season_number: season_episode.map(|value| value.0),
        episode_number: season_episode.map(|value| value.1),
    }
}

fn parse_bracket_animation(value: &str) -> Option<(String, i32)> {
    let bracket = Regex::new(r"[\[【]([^\]】]+)[\]】]").expect("valid bracket token regex");
    let resolution =
        Regex::new(r"(?i)^(?:480|720|1080|2160)p$").expect("valid resolution token regex");
    let episode_regex =
        Regex::new(r"(?i)^0*(\d{1,3})(?:v\d+)?$").expect("valid bracket episode regex");
    let tokens = bracket
        .captures_iter(value)
        .filter_map(|captures| captures.get(1))
        .map(|token| token.as_str().trim())
        .collect::<Vec<_>>();

    if tokens.len() < 3 || !tokens.iter().any(|token| resolution.is_match(token)) {
        return None;
    }

    let (episode_index, episode) = tokens.iter().enumerate().find_map(|(index, token)| {
        let captures = episode_regex.captures(token)?;
        let episode = captures.get(1)?.as_str().parse::<i32>().ok()?;
        (index > 0 && episode > 0).then_some((index, episode))
    })?;
    let title = tokens[..episode_index]
        .iter()
        .rev()
        .find(|token| !is_technical_bracket_token(token))?;
    let title = normalize(title);

    (!title.is_empty()).then_some((title, episode))
}

fn is_technical_bracket_token(value: &str) -> bool {
    Regex::new(
        r"(?i)^(?:480p|720p|1080p|2160p|4k|web-?dl|webrip|bluray|bdrip|x26[45]|h\.?26[45]|hevc|av1|aac|flac|jptc|cht|chs|sc|tc)$",
    )
    .expect("valid technical bracket token regex")
    .is_match(value)
}

pub fn normalized_group_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn normalize(value: &str) -> String {
    let separators = Regex::new(r"[._]+").expect("valid separator regex");
    let brackets = Regex::new(r"[\[\]{}【】（）()]+").expect("valid bracket regex");
    let whitespace = Regex::new(r"\s+").expect("valid whitespace regex");

    let separated = separators.replace_all(value, " ");
    let unwrapped = brackets.replace_all(&separated, " ");

    whitespace.replace_all(&unwrapped, " ").trim().to_string()
}

fn parse_year(value: &str) -> Option<i32> {
    let regex = Regex::new(r"(?i)(?:^|\D)((?:19|20)\d{2})(?:\D|$)").expect("valid year regex");
    regex
        .captures(value)
        .and_then(|captures| captures.get(1))
        .and_then(|year| year.as_str().parse().ok())
}

fn parse_season_episode(value: &str) -> Option<(i32, i32)> {
    let standard =
        Regex::new(r"(?i)(?:^|\D)S(\d{1,2})\s*E(\d{1,3})(?:\D|$)").expect("valid episode regex");
    if let Some(captures) = standard.captures(value) {
        return Some((
            captures.get(1)?.as_str().parse().ok()?,
            captures.get(2)?.as_str().parse().ok()?,
        ));
    }

    let chinese = Regex::new(r"第\s*(\d{1,2})\s*季.*?第\s*(\d{1,3})\s*集")
        .expect("valid Chinese episode regex");
    chinese
        .captures(value)
        .and_then(|captures| {
            Some((
                captures.get(1)?.as_str().parse().ok()?,
                captures.get(2)?.as_str().parse().ok()?,
            ))
        })
        .or_else(|| {
            let alternate = Regex::new(r"(?i)(?:^|\D)(\d{1,2})x(\d{1,3})(?:\D|$)")
                .expect("valid x episode regex");
            alternate.captures(value).and_then(|captures| {
                Some((
                    captures.get(1)?.as_str().parse().ok()?,
                    captures.get(2)?.as_str().parse().ok()?,
                ))
            })
        })
}

fn parse_loose_animation_episode(value: &str) -> Option<(i32, i32)> {
    let explicit = Regex::new(r"(?i)(?:^|\s)(?:EP?|Episode)\s*0*(\d{1,3})(?:\s|$)")
        .expect("valid loose episode regex");
    if let Some(captures) = explicit.captures(value) {
        return Some((1, captures.get(1)?.as_str().parse().ok()?));
    }

    let chinese =
        Regex::new(r"第\s*0*(\d{1,3})\s*[集话話]").expect("valid Chinese loose episode regex");
    if let Some(captures) = chinese.captures(value) {
        return Some((1, captures.get(1)?.as_str().parse().ok()?));
    }

    let trailing = Regex::new(r"(?i)(?:^|\s-\s|\s)0*(\d{1,3})(?:v\d+)?(?:\s|$)")
        .expect("valid trailing episode regex");
    trailing
        .captures_iter(value)
        .last()
        .and_then(|captures| Some((1, captures.get(1)?.as_str().parse().ok()?)))
}

fn clean_title(value: &str, year: Option<i32>, season_episode: Option<(i32, i32)>) -> String {
    let mut title = value.to_string();

    if let Some(year) = year {
        title = Regex::new(&format!(r"(?i)(?:^|\s){}(?:\s|$).*$", year))
            .expect("valid dynamic year regex")
            .replace(&title, "")
            .to_string();
    }

    if season_episode.is_some() {
        title = Regex::new(r"(?i)\s*S\d{1,2}\s*E\d{1,3}.*$")
            .expect("valid episode cleanup regex")
            .replace(&title, "")
            .to_string();
        title = Regex::new(r"\s*第\s*\d{1,2}\s*季.*$")
            .expect("valid Chinese episode cleanup regex")
            .replace(&title, "")
            .to_string();
    }

    let release_tokens = Regex::new(
        r"(?i)\b(2160p|1080p|720p|480p|bluray|blu-ray|web-dl|webrip|hdtv|remux|x264|x265|h\.?264|h\.?265|hevc|av1|hdr10\+?|dolby\s*vision|dv|aac|dts|truehd|atmos)\b.*$",
    )
    .expect("valid release token regex");
    title = release_tokens.replace(&title, "").to_string();

    let cleaned = title.trim_matches([' ', '-', '.']).trim();
    if cleaned.is_empty() {
        value.to_string()
    } else {
        cleaned.to_string()
    }
}

fn path_looks_like_animation(path: &Path, source_root: &Path) -> bool {
    let relative_path = path.strip_prefix(source_root).unwrap_or(path);
    let lower = relative_path.to_string_lossy().to_lowercase();
    let keywords = [
        "anime",
        "animation",
        "bangumi",
        "cartoon",
        "动画",
        "動漫",
        "动漫",
        "番剧",
        "番劇",
    ];
    if keywords.iter().any(|keyword| lower.contains(keyword)) {
        return true;
    }

    source_root
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::to_lowercase)
        .map(|name| keywords.contains(&name.as_str()))
        .unwrap_or(false)
}

fn series_parent_title(path: &Path, source_root: &Path) -> Option<String> {
    let parent = path.parent()?;
    if parent == source_root {
        return None;
    }
    let parent_name = parent.file_name()?.to_str()?;
    let season_folder =
        Regex::new(r"(?i)^(?:season\s*\d+|s\d{1,2}|第\s*\d+\s*季)$").expect("season folder regex");

    let candidate = if season_folder.is_match(parent_name) {
        parent
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            .unwrap_or(parent_name)
    } else {
        parent_name
    };

    let cleaned = normalize(candidate);
    (!cleaned.is_empty()).then_some(cleaned)
}

fn looks_like_release_group_only(value: &str) -> bool {
    let lower = value.to_lowercase();
    ["web dl", "bluray", "1080p", "2160p", "hevc", "x264", "x265"]
        .iter()
        .any(|token| lower == *token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_movie() {
        let parsed = parse_media_name(
            Path::new("Interstellar.2014.2160p.BluRay.mkv"),
            Path::new("."),
        );
        assert_eq!(parsed.title, "Interstellar");
        assert_eq!(parsed.year, Some(2014));
        assert_eq!(parsed.media_type, "movie");
    }

    #[test]
    fn parses_episode() {
        let parsed = parse_media_name(Path::new("Frieren.S01E03.1080p.mkv"), Path::new("."));
        assert_eq!(parsed.title, "Frieren");
        assert_eq!(parsed.season_number, Some(1));
        assert_eq!(parsed.episode_number, Some(3));
        assert_eq!(parsed.media_type, "series");
    }

    #[test]
    fn detects_animation_from_path() {
        let parsed = parse_media_name(Path::new(r"D:\Animation\Frieren\03.mkv"), Path::new(r"D:\"));
        assert_eq!(parsed.title, "Frieren");
        assert_eq!(parsed.media_type, "animation");
    }

    #[test]
    fn source_folder_name_does_not_force_every_item_to_animation() {
        let parsed = parse_media_name(
            Path::new(r"D:\动画影视资源\电影\Eat.Drink.Man.Woman.1994.mkv"),
            Path::new(r"D:\动画影视资源"),
        );
        assert_eq!(parsed.media_type, "movie");
    }

    #[test]
    fn groups_chinese_animation_by_parent_folder() {
        let parsed = parse_media_name(
            Path::new(r"D:\动画\葬送的芙莉莲\03 [1080p].mkv"),
            Path::new(r"D:\动画"),
        );
        assert_eq!(parsed.title, "葬送的芙莉莲");
        assert_eq!(parsed.episode_number, Some(3));
        assert_eq!(parsed.group_key, "葬送的芙莉莲");
    }

    #[test]
    fn groups_named_animation_episode_by_parent_folder() {
        let parsed = parse_media_name(
            Path::new(r"D:\Anime\Frieren\Frieren - 03 [1080p].mkv"),
            Path::new(r"D:\Anime"),
        );
        assert_eq!(parsed.title, "Frieren");
        assert_eq!(parsed.episode_number, Some(3));
        assert_eq!(parsed.group_key, "frieren");
    }

    #[test]
    fn parses_subtitle_group_bracket_animation() {
        let parsed = parse_media_name(
            Path::new(r"D:\Downloads\[Nekomoe kissaten][LAZARUS][01][1080p][JPTC].mkv"),
            Path::new(r"D:\Downloads"),
        );
        assert_eq!(parsed.title, "LAZARUS");
        assert_eq!(parsed.media_type, "animation");
        assert_eq!(parsed.season_number, Some(1));
        assert_eq!(parsed.episode_number, Some(1));
        assert_eq!(parsed.group_key, "lazarus");
    }
}
