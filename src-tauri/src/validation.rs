use crate::models::Platform;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use url::Url;

pub fn validate_media_url(input: &str) -> Result<(Url, Platform), String> {
    let mut parsed =
        Url::parse(input.trim()).map_err(|_| "網址格式不正確。請貼上完整的 https:// 網址。")?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("僅支援 http 或 https 網址。".into());
    }
    let host = parsed
        .host_str()
        .unwrap_or_default()
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    let path = parsed.path().to_ascii_lowercase();
    if matches!(
        host.as_str(),
        "youtube.com" | "m.youtube.com" | "music.youtube.com" | "youtu.be"
    ) {
        if path.starts_with("/playlist") {
            return Err("第一版不支援 YouTube 播放清單。".into());
        }
        if path.starts_with("/live/") {
            return Err("第一版不支援直播下載。".into());
        }
        return Ok((parsed, Platform::Youtube));
    }
    if matches!(host.as_str(), "instagram.com" | "m.instagram.com") {
        if path.starts_with("/stories/") {
            return Err("第一版不支援 Instagram Stories。".into());
        }
        if !(path.starts_with("/p/") || path.starts_with("/reel/") || path.starts_with("/reels/")) {
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
        let _ = parsed.set_username("");
        let _ = parsed.set_password(None);
        parsed.set_path(&canonical_path);
        parsed.set_query(None);
        parsed.set_fragment(None);
        return Ok((parsed, Platform::Instagram));
    }
    Err("目前只支援 Instagram 與 YouTube 網址。".into())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_urls() {
        assert_eq!(
            validate_media_url("https://youtu.be/abc").unwrap().1,
            Platform::Youtube
        );
        assert_eq!(
            validate_media_url("https://www.instagram.com/reel/abc/")
                .unwrap()
                .1,
            Platform::Instagram
        );
    }
    #[test]
    fn canonicalizes_instagram_share_urls() {
        let (url, platform) = validate_media_url(
            "https://www.instagram.com/p/DdEHbTEn1jz/?utm_source=ig_web_copy_link&stkn=token#fragment",
        )
        .unwrap();
        assert_eq!(platform, Platform::Instagram);
        assert_eq!(url.as_str(), "https://www.instagram.com/p/DdEHbTEn1jz/");
    }
    #[test]
    fn rejects_unsupported_and_local_urls() {
        assert!(validate_media_url("file:///c:/secret").is_err());
        assert!(validate_media_url("https://example.com/video").is_err());
        assert!(validate_media_url("https://youtube.com/playlist?list=123").is_err());
        assert!(validate_media_url("https://instagram.com/stories/name/1").is_err());
    }
    #[test]
    fn cleans_windows_names() {
        assert_eq!(safe_title("CON"), "_CON");
        assert_eq!(safe_title("a<b>:c?. "), "a_b__c_");
        assert_eq!(safe_title(""), "media");
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
