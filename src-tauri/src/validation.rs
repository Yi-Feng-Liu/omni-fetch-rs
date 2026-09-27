use crate::models::Platform;
use std::{
    fs,
    io::Read,
    net::IpAddr,
    path::{Path, PathBuf},
};
use url::Url;

pub fn validate_media_url(input: &str, platform: Platform) -> Result<Url, String> {
    let mut parsed =
        Url::parse(input.trim()).map_err(|_| "網址格式不正確。請貼上完整的 https:// 網址。")?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("僅支援 http 或 https 網址。".into());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("網址不可包含帳號或密碼。".into());
    }
    let host = parsed
        .host_str()
        .unwrap_or_default()
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    let path = parsed.path().to_ascii_lowercase();
    match platform {
        Platform::Youtube => {
            if !matches!(
                host.as_str(),
                "youtube.com" | "m.youtube.com" | "music.youtube.com" | "youtu.be"
            ) {
                return Err("目前是 YouTube 模式，請貼上 YouTube 影片或 Shorts 網址。".into());
            }
            if path.starts_with("/playlist") {
                return Err("第一版不支援 YouTube 播放清單。".into());
            }
            if path.starts_with("/live/") {
                return Err("第一版不支援直播下載。".into());
            }
        }
        Platform::Instagram => {
            if !matches!(host.as_str(), "instagram.com" | "m.instagram.com") {
                return Err("目前是 Instagram 模式，請貼上 Instagram 貼文或 Reels 網址。".into());
            }
            if path.starts_with("/stories/") {
                return Err("第一版不支援 Instagram Stories。".into());
            }
            if !(path.starts_with("/p/")
                || path.starts_with("/reel/")
                || path.starts_with("/reels/"))
            {
                return Err("請貼上 Instagram 貼文或 Reels 網址。".into());
            }
            let segments: Vec<_> = parsed
                .path_segments()
                .into_iter()
                .flatten()
                .filter(|segment| !segment.is_empty())
                .take(2)
                .collect();
            if segments.len() != 2 {
                return Err("Instagram 網址缺少貼文識別碼。".into());
            }
            let canonical_path = format!("/{}/{}/", segments[0], segments[1]);
            let _ = parsed.set_scheme("https");
            parsed
                .set_host(Some("www.instagram.com"))
                .map_err(|_| "Instagram 網址格式不正確。")?;
            parsed.set_path(&canonical_path);
            parsed.set_query(None);
            parsed.set_fragment(None);
        }
        Platform::Web => {
            if host.is_empty() || host == "localhost" || host.ends_with(".localhost") {
                return Err("一般網頁模式只接受公開的 http 或 https 網址。".into());
            }
            if host.parse::<IpAddr>().is_ok_and(is_private_address) {
                return Err("一般網頁模式不接受本機或私人網路位址。".into());
            }
            if path.ends_with(".ts") {
                return Err("這是單一影片片段；請改貼影片網頁或 .m3u8 播放清單網址。".into());
            }
        }
    }
    Ok(parsed)
}

pub fn validate_source_page_url(input: Option<&str>) -> Result<Option<String>, String> {
    let Some(input) = input.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let parsed = validate_media_url(input, Platform::Web)
        .map_err(|_| "來源頁網址格式不正確，請貼上完整的公開 http 或 https 網址。")?;
    Ok(Some(parsed.to_string()))
}

pub fn resolve_web_media_url(
    media_url: Url,
    source_page_url: Option<String>,
) -> Result<(Url, Option<String>), String> {
    let host = media_url
        .host_str()
        .unwrap_or_default()
        .trim_start_matches("www.")
        .to_ascii_lowercase();

    if host == "18porn.cc" {
        if let Some(id) = media_url
            .path()
            .strip_prefix("/video/")
            .and_then(|path| path.strip_suffix(".html"))
            .filter(|id| !id.is_empty() && id.chars().all(|character| character.is_ascii_digit()))
        {
            let manifest = Url::parse(&format!("https://cdn.18porn.cc/videos/{id}/{id}.m3u8"))
                .map_err(|_| "無法建立影片播放清單網址。")?;
            let referer = source_page_url.or_else(|| Some(media_url.to_string()));
            return Ok((manifest, referer));
        }
    }

    if host == "cdn.18porn.cc" {
        let segments: Vec<_> = media_url.path_segments().into_iter().flatten().collect();
        if let ["videos", id, filename] = segments.as_slice() {
            let expected = format!("{id}.m3u8");
            if !id.is_empty()
                && id.chars().all(|character| character.is_ascii_digit())
                && filename.eq_ignore_ascii_case(&expected)
            {
                let referer =
                    source_page_url.or_else(|| Some(format!("https://18porn.cc/video/{id}.html")));
                return Ok((media_url, referer));
            }
        }
    }

    Ok((media_url, source_page_url))
}

fn is_private_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => {
            ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified()
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
        }
    }
}

pub fn validate_output_directory(input: &str) -> Result<PathBuf, String> {
    let path = Path::new(input);
    if input.trim().is_empty() || !path.is_absolute() {
        return Err("請選擇有效的絕對儲存路徑。".into());
    }
    if !path.is_dir() {
        return Err("儲存資料夾不存在或無法存取。".into());
    }
    Ok(path.to_path_buf())
}

pub fn validate_cookie_file(input: &str) -> Result<PathBuf, String> {
    let path = Path::new(input);
    if input.trim().is_empty() || !path.is_absolute() {
        return Err("請選擇有效的 cookies.txt 絕對路徑。".into());
    }
    let metadata = fs::metadata(path).map_err(|_| "找不到 cookies.txt，請重新選擇檔案。")?;
    if !metadata.is_file() {
        return Err("選擇的 Cookie 路徑不是檔案。".into());
    }
    if metadata.len() > 10 * 1024 * 1024 {
        return Err("cookies.txt 檔案異常過大，已拒絕讀取。".into());
    }

    let mut file = fs::File::open(path).map_err(|_| "無法讀取 cookies.txt。")?;
    let mut bytes = Vec::with_capacity(metadata.len().min(10 * 1024 * 1024) as usize);
    file.read_to_end(&mut bytes)
        .map_err(|_| "無法讀取 cookies.txt。")?;
    let text = String::from_utf8(bytes).map_err(|_| "cookies.txt 必須是 UTF-8 文字檔。")?;
    let text = text.trim_start_matches('\u{feff}');
    let header_is_valid = text.lines().next().is_some_and(|line| {
        matches!(
            line.trim(),
            "# Netscape HTTP Cookie File" | "# HTTP Cookie File"
        )
    });
    let contains_cookie = text.lines().skip(1).any(|line| {
        let candidate = line.trim();
        (!candidate.starts_with('#') || candidate.starts_with("#HttpOnly_"))
            && candidate.split('\t').count() >= 7
    });
    if !header_is_valid || !contains_cookie {
        return Err(
            "Cookie 檔案格式不正確或沒有 Cookie；請匯出 Netscape 格式的 cookies.txt。".into(),
        );
    }
    path.canonicalize()
        .map_err(|_| "無法解析 cookies.txt 路徑。".into())
}

pub fn safe_title(input: &str) -> String {
    let invalid = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut value: String = input
        .chars()
        .map(|c| {
            if invalid.contains(&c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    value = value
        .trim()
        .trim_end_matches(['.', ' '])
        .chars()
        .take(120)
        .collect();
    let upper = value.to_ascii_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if value.is_empty() {
        "media".into()
    } else if reserved.contains(&upper.as_str()) {
        format!("_{value}")
    } else {
        value
    }
}

pub fn validate_output_filename(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("請輸入儲存檔名。".into());
    }
    let lower = trimmed.to_ascii_lowercase();
    let without_extension = [".mp4", ".mp3", ".webm", ".mkv", ".m4a", ".mov", ".ts"]
        .iter()
        .find_map(|extension| {
            lower
                .ends_with(extension)
                .then(|| &trimmed[..trimmed.len() - extension.len()])
        })
        .unwrap_or(trimmed);
    if without_extension.trim().is_empty() {
        return Err("請輸入有效的儲存檔名。".into());
    }
    Ok(safe_title(without_extension))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_urls() {
        assert!(validate_media_url("https://youtu.be/abc", Platform::Youtube).is_ok());
        assert!(
            validate_media_url("https://www.instagram.com/reel/abc/", Platform::Instagram).is_ok()
        );
        assert!(validate_media_url("https://example.com/watch/123", Platform::Web).is_ok());
        assert!(
            validate_media_url("https://cdn.example.com/video/master.m3u8", Platform::Web).is_ok()
        );
    }
    #[test]
    fn canonicalizes_instagram_share_urls() {
        let url = validate_media_url(
            "https://www.instagram.com/p/DdEHbTEn1jz/?utm_source=ig_web_copy_link&stkn=token#fragment",
            Platform::Instagram,
        )
        .unwrap();
        assert_eq!(url.as_str(), "https://www.instagram.com/p/DdEHbTEn1jz/");
    }
    #[test]
    fn rejects_unsupported_and_local_urls() {
        assert!(validate_media_url("file:///c:/secret", Platform::Web).is_err());
        assert!(validate_media_url("https://example.com/video", Platform::Youtube).is_err());
        assert!(
            validate_media_url("https://youtube.com/playlist?list=123", Platform::Youtube).is_err()
        );
        assert!(
            validate_media_url("https://instagram.com/stories/name/1", Platform::Instagram)
                .is_err()
        );
        assert!(validate_media_url("http://127.0.0.1/video.m3u8", Platform::Web).is_err());
        assert!(validate_media_url("https://cdn.example.com/file-001.ts", Platform::Web).is_err());
    }

    #[test]
    fn validates_optional_source_page() {
        assert_eq!(validate_source_page_url(None).unwrap(), None);
        assert!(
            validate_source_page_url(Some("https://example.com/watch/123"))
                .unwrap()
                .is_some()
        );
        assert!(validate_source_page_url(Some("file:///c:/secret")).is_err());
    }

    #[test]
    fn resolves_known_web_page_and_manifest_patterns() {
        let page = validate_media_url("https://18porn.cc/video/1953.html", Platform::Web).unwrap();
        let (manifest, referer) = resolve_web_media_url(page, None).unwrap();
        assert_eq!(
            manifest.as_str(),
            "https://cdn.18porn.cc/videos/1953/1953.m3u8"
        );
        assert_eq!(
            referer.as_deref(),
            Some("https://18porn.cc/video/1953.html")
        );

        let direct =
            validate_media_url("https://cdn.18porn.cc/videos/5378/5378.m3u8", Platform::Web)
                .unwrap();
        let (manifest, referer) = resolve_web_media_url(direct, None).unwrap();
        assert_eq!(
            manifest.as_str(),
            "https://cdn.18porn.cc/videos/5378/5378.m3u8"
        );
        assert_eq!(
            referer.as_deref(),
            Some("https://18porn.cc/video/5378.html")
        );
    }

    #[test]
    fn leaves_unknown_web_urls_unchanged() {
        let page = validate_media_url("https://example.com/watch/123", Platform::Web).unwrap();
        let (resolved, referer) = resolve_web_media_url(page.clone(), None).unwrap();
        assert_eq!(resolved, page);
        assert_eq!(referer, None);
    }
    #[test]
    fn cleans_windows_names() {
        assert_eq!(safe_title("CON"), "_CON");
        assert_eq!(safe_title("a<b>:c?. "), "a_b__c_");
        assert_eq!(safe_title(""), "media");
    }

    #[test]
    fn validates_custom_output_filename() {
        assert_eq!(
            validate_output_filename("My video.mp4").unwrap(),
            "My video"
        );
        assert_eq!(
            validate_output_filename("chapter: 1").unwrap(),
            "chapter_ 1"
        );
        assert!(validate_output_filename("  ").is_err());
    }

    #[test]
    fn validates_netscape_cookie_files() {
        let dir = tempfile::tempdir().unwrap();
        let valid = dir.path().join("cookies.txt");
        fs::write(
            &valid,
            "# Netscape HTTP Cookie File\n.instagram.com\tTRUE\t/\tTRUE\t0\tsessionid\tsecret\n",
        )
        .unwrap();
        assert!(validate_cookie_file(valid.to_str().unwrap()).is_ok());

        let invalid = dir.path().join("invalid.txt");
        fs::write(&invalid, "sessionid=secret").unwrap();
        assert!(validate_cookie_file(invalid.to_str().unwrap()).is_err());
    }
}
