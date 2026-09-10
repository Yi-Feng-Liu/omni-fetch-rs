use crate::{
    errors::friendly_error,
    models::{AnalyzeRequest, MediaAnalysis, MediaItem, OutputFormat, Platform, QualityOption},
    tools,
    validation::validate_media_url,
};
use serde_json::Value;
use std::collections::BTreeMap;
use tauri::AppHandle;
use tokio::time::{timeout, Duration};

pub async fn analyze(app: &AppHandle, request: AnalyzeRequest) -> Result<MediaAnalysis, String> {
    let (parsed, platform) = validate_media_url(&request.url)?;
    if platform != request.platform {
        return Err(match request.platform {
            Platform::Instagram => "目前是 Instagram 模式，請貼上 Instagram 貼文或 Reels 網址。",
            Platform::Youtube => "目前是 YouTube 模式，請貼上 YouTube 影片或 Shorts 網址。",
        }
        .into());
    }
    if platform == Platform::Instagram {
        return analyze_instagram(app, request, parsed.to_string()).await;
    }
    let mut command = tools::ytdlp_command(
        app,
        request.browser_source,
        request.cookie_file_path.as_deref(),
    )?;
    command.args(["--dump-single-json", "--skip-download", "--no-warnings"]);
    command.arg("--no-playlist");
    command.arg(parsed.as_str());
    let output = timeout(Duration::from_secs(90), command.output())
        .await
        .map_err(|_| "分析逾時，請確認網路連線後重試。")?
        .map_err(|_| "找不到下載引擎。請執行工具準備腳本或重新安裝 Omni Fetch。")?;
    if !output.status.success() {
        return Err(friendly_error(&String::from_utf8_lossy(&output.stderr)));
    }
    let json: Value =
        serde_json::from_slice(&output.stdout).map_err(|_| "下載引擎回傳了無法辨識的媒體資訊。")?;
    if json
        .get("is_live")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || json
            .get("live_status")
            .and_then(Value::as_str)
            .is_some_and(|s| s != "not_live")
    {
        return Err("第一版不支援直播下載。".into());
    }
    from_json(parsed.to_string(), platform, &json)
}

async fn analyze_instagram(
    app: &AppHandle,
    request: AnalyzeRequest,
    url: String,
) -> Result<MediaAnalysis, String> {
    let mut command = tools::gallery_command(
        app,
        request.browser_source,
        request.cookie_file_path.as_deref(),
    )?;
    command.args([
        "--config-ignore",
        "--no-input",
        "--no-colors",
        "--dump-json",
        "--simulate",
        &url,
    ]);
    let output = timeout(Duration::from_secs(90), command.output())
        .await
        .map_err(|_| "Instagram 分析逾時，請確認網路連線後重試。")?
        .map_err(|_| "找不到 Instagram 圖片引擎，請重新安裝 Omni Fetch。")?;
    if !output.status.success() {
        return Err(friendly_error(&String::from_utf8_lossy(&output.stderr)));
    }
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "Instagram 引擎回傳了無法辨識的媒體資訊。")?;
    from_gallery_json(url, &json)
}

fn gallery_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| string(value, key))
}

fn extension_from_url(url: &str) -> String {
    url.split('?')
        .next()
        .and_then(|path| path.rsplit('.').next())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn from_gallery_json(url: String, json: &Value) -> Result<MediaAnalysis, String> {
    let records = json.as_array().ok_or("Instagram 引擎回傳格式不正確。")?;
    let mut gallery_metadata: Option<&Value> = None;
    let mut items = Vec::new();
    let mut engine_error = None;

    for record in records.iter().filter_map(Value::as_array) {
        let kind = record.first().and_then(Value::as_i64).unwrap_or_default();
        let metadata = record.get(2).or_else(|| record.get(1));
        match kind {
            -1 => {
                engine_error =
                    metadata.and_then(|value| gallery_string(value, &["message", "error"]));
            }
            2 => gallery_metadata = metadata,
            3 => {
                let Some(media_url) = record.get(1).and_then(Value::as_str) else {
                    continue;
                };
                let metadata = record.get(2).unwrap_or(json);
                let extension = gallery_string(metadata, &["extension", "ext"])
                    .unwrap_or_else(|| extension_from_url(media_url));
                let media_type =
                    if matches!(extension.as_str(), "mp4" | "webm" | "mov" | "m4v" | "mkv") {
                        "video"
                    } else {
                        "image"
                    };
                let index = items.len() + 1;
                let id = gallery_string(
                    metadata,
                    &["post_shortcode", "shortcode", "post_id", "media_id", "id"],
                )
                .unwrap_or_else(|| format!("instagram-{index}"));
                let title = gallery_string(metadata, &["description", "caption", "title"])
                    .unwrap_or_else(|| format!("Instagram 媒體 {index}"));
                let thumbnail = if media_type == "image" {
                    Some(media_url.to_string())
                } else {
                    gallery_string(metadata, &["thumbnail", "display_url"])
                };
                items.push(MediaItem {
                    id,
                    title,
                    media_type: media_type.into(),
                    thumbnail,
                    duration_seconds: metadata.get("duration").and_then(Value::as_f64),
                });
            }
            _ => {}
        }
    }

    if items.is_empty() {
        return Err(engine_error
            .map(|message| friendly_error(&message))
            .unwrap_or_else(|| "這個 Instagram 網址沒有可下載的圖片或影片。".into()));
    }
    let metadata = gallery_metadata.unwrap_or(json);
    let title = gallery_string(metadata, &["description", "caption", "title"])
        .or_else(|| items.first().map(|item| item.title.clone()))
        .unwrap_or_else(|| "Instagram 貼文".into());
    let uploader = gallery_string(metadata, &["username", "owner_username", "user"]);
    let thumbnail = items.first().and_then(|item| item.thumbnail.clone());
    Ok(MediaAnalysis {
        url,
        platform: Platform::Instagram,
        title,
        thumbnail,
        uploader,
        duration_seconds: None,
        is_carousel: items.len() > 1,
        items,
        qualities: Vec::new(),
        supported_outputs: vec![OutputFormat::Original],
    })
}

fn string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn media_item(value: &Value) -> MediaItem {
    let ext = string(value, "ext").unwrap_or_default();
    let media_type = if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") {
        "image"
    } else if value
        .get("vcodec")
        .and_then(Value::as_str)
        .is_some_and(|v| v != "none")
        || value.get("duration").is_some()
    {
        "video"
    } else {
        "image"
    };
    MediaItem {
        id: string(value, "id").unwrap_or_else(|| "media".into()),
        title: string(value, "title").unwrap_or_else(|| "未命名媒體".into()),
        media_type: media_type.into(),
        thumbnail: string(value, "thumbnail"),
        duration_seconds: value.get("duration").and_then(Value::as_f64),
    }
}

fn collect_qualities(value: &Value, result: &mut BTreeMap<u32, Option<u64>>) {
    let Some(formats) = value.get("formats").and_then(Value::as_array) else {
        return;
    };
    for format in formats {
        let Some(height) = format
            .get("height")
            .and_then(Value::as_u64)
            .filter(|h| *h > 0)
        else {
            continue;
        };
        let has_video = format
            .get("vcodec")
            .and_then(Value::as_str)
            .is_some_and(|v| v != "none");
        if !has_video {
            continue;
        }
        let size = format
            .get("filesize")
            .or_else(|| format.get("filesize_approx"))
            .and_then(Value::as_u64);
        result
            .entry(height as u32)
            .and_modify(|current| {
                if current.is_none() {
                    *current = size;
                }
            })
            .or_insert(size);
    }
}

fn from_json(url: String, platform: Platform, json: &Value) -> Result<MediaAnalysis, String> {
    let entries = json.get("entries").and_then(Value::as_array);
    let items: Vec<_> = entries
        .map(|values| {
            values
                .iter()
                .filter(|v| !v.is_null())
                .map(media_item)
                .collect()
        })
        .unwrap_or_else(|| vec![media_item(json)]);
    if items.is_empty() {
        return Err("這個網址沒有可下載的媒體。".into());
    }
    let mut quality_map = BTreeMap::new();
    collect_qualities(json, &mut quality_map);
    if let Some(values) = entries {
        for value in values {
            collect_qualities(value, &mut quality_map);
        }
    }
    let qualities = quality_map
        .into_iter()
        .map(|(height, estimated_bytes)| QualityOption {
            height,
            label: format!("{height}p"),
            estimated_bytes,
        })
        .collect();
    let title = string(json, "title")
        .or_else(|| items.first().map(|i| i.title.clone()))
        .unwrap_or_else(|| "未命名媒體".into());
    Ok(MediaAnalysis {
        url,
        platform,
        title,
        thumbnail: string(json, "thumbnail")
            .or_else(|| items.first().and_then(|i| i.thumbnail.clone())),
        uploader: string(json, "uploader").or_else(|| string(json, "channel")),
        duration_seconds: json.get("duration").and_then(Value::as_f64),
        is_carousel: items.len() > 1,
        items,
        qualities,
        supported_outputs: if platform == Platform::Youtube {
            vec![OutputFormat::Mp4, OutputFormat::Mp3, OutputFormat::Original]
        } else {
            vec![OutputFormat::Original]
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_video_and_qualities() {
        let json: Value = serde_json::json!({"id":"x","title":"Demo","duration":10.0,"formats":[
            {"height":720,"vcodec":"avc1","filesize":1000}, {"height":1080,"vcodec":"av01"}, {"acodec":"opus","vcodec":"none"}
        ]});
        let result = from_json("https://youtu.be/x".into(), Platform::Youtube, &json).unwrap();
        assert_eq!(result.qualities.len(), 2);
        assert_eq!(result.items[0].media_type, "video");
        assert_eq!(result.supported_outputs.len(), 3);
    }
    #[test]
    fn maps_carousel() {
        let json = serde_json::json!({"title":"Post","entries":[{"id":"1","ext":"jpg"},{"id":"2","ext":"mp4","duration":2}]});
        let result = from_json(
            "https://instagram.com/p/x".into(),
            Platform::Instagram,
            &json,
        )
        .unwrap();
        assert!(result.is_carousel);
        assert_eq!(result.items.len(), 2);
    }

    #[test]
    fn maps_gallery_images_and_videos() {
        let json = serde_json::json!([
            [2, null, {"description":"Demo post","username":"creator"}],
            [3, "https://cdn.example/one.jpg", {"extension":"jpg","post_id":"1"}],
            [3, "https://cdn.example/two.mp4", {"extension":"mp4","post_id":"2","thumbnail":"https://cdn.example/two.jpg"}]
        ]);
        let result = from_gallery_json("https://instagram.com/p/demo/".into(), &json).unwrap();
        assert!(result.is_carousel);
        assert_eq!(result.items[0].media_type, "image");
        assert_eq!(result.items[1].media_type, "video");
        assert_eq!(result.uploader.as_deref(), Some("creator"));
    }

    #[test]
    fn maps_gallery_login_errors() {
        let json = serde_json::json!([[-1, {"error":"AbortExtraction","message":"HTTP redirect to login page"}]]);
        assert!(
            from_gallery_json("https://instagram.com/p/demo/".into(), &json)
                .unwrap_err()
                .contains("登入")
        );
    }
}
