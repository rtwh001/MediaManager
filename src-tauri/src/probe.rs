use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

use serde_json::Value;
use tauri::{path::BaseDirectory, AppHandle, Manager};

static FFPROBE_PATH: OnceLock<PathBuf> = OnceLock::new();

#[derive(Default)]
pub struct MediaProbe {
    pub duration_seconds: Option<f64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub container_format: Option<String>,
    pub hdr_format: Option<String>,
}

pub fn initialize(app: &AppHandle) {
    let resource = app
        .path()
        .resolve("binaries/ffprobe.exe", BaseDirectory::Resource)
        .ok()
        .filter(|path| path.is_file());
    let development = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join("ffprobe.exe");
    let path = resource
        .or_else(|| development.is_file().then_some(development))
        .unwrap_or_else(|| PathBuf::from("ffprobe"));
    let _ = FFPROBE_PATH.set(path);
}

fn ffprobe_command() -> Command {
    Command::new(
        FFPROBE_PATH
            .get()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("ffprobe")),
    )
}

pub fn ffprobe_version() -> Option<String> {
    ffprobe_command()
        .arg("-version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|output| output.lines().next().map(str::to_string))
}

pub fn probe_media(path: &Path) -> Result<MediaProbe, String> {
    let output = ffprobe_command()
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration,format_name:stream=codec_type,codec_name,width,height,color_transfer,color_primaries",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|error| format!("无法启动 ffprobe: {error}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("无法解析 ffprobe 输出: {error}"))?;
    let streams = value
        .get("streams")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let video = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"));
    let audio = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio"));
    let format = value.get("format");

    let color_transfer = video
        .and_then(|stream| stream.get("color_transfer"))
        .and_then(Value::as_str);
    let hdr_format = match color_transfer {
        Some("smpte2084") => Some("HDR10".to_string()),
        Some("arib-std-b67") => Some("HLG".to_string()),
        _ => None,
    };

    Ok(MediaProbe {
        duration_seconds: format
            .and_then(|item| item.get("duration"))
            .and_then(Value::as_str)
            .and_then(|duration| duration.parse().ok()),
        width: video
            .and_then(|stream| stream.get("width"))
            .and_then(Value::as_i64),
        height: video
            .and_then(|stream| stream.get("height"))
            .and_then(Value::as_i64),
        video_codec: video
            .and_then(|stream| stream.get("codec_name"))
            .and_then(Value::as_str)
            .map(str::to_string),
        audio_codec: audio
            .and_then(|stream| stream.get("codec_name"))
            .and_then(Value::as_str)
            .map(str::to_string),
        container_format: format
            .and_then(|item| item.get("format_name"))
            .and_then(Value::as_str)
            .map(str::to_string),
        hdr_format,
    })
}
