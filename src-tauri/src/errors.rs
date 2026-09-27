use crate::models::Platform;

pub fn friendly_error_for_request(
    stderr: &str,
    platform: Platform,
    has_source_page: bool,
) -> String {
    let text = stderr.to_ascii_lowercase();
    if text.contains("drm") {
        return "此影片受到 DRM 保護，Omni Fetch 不支援下載或繞過保護。".into();
    }
    if platform == Platform::Web
        && (text.contains("403")
            || text.contains("forbidden")
            || text.contains("cloudflare anti-bot"))
    {
        return if has_source_page {
            "網站仍拒絕存取。請確認來源頁網址與 Cookie 有效，或改用瀏覽器匯出的 cookies.txt。"
                .into()
        } else {
            "網站拒絕直接存取。請在「來源頁網址」填入實際播放影片的網頁後重試。".into()
        };
    }
    if platform == Platform::Web
        && (text.contains("unsupported url") || text.contains("no suitable extractor"))
    {
        return "無法從這個網頁找出影片；請改貼播放器使用的 .m3u8 播放清單網址。".into();
    }
    friendly_error(stderr)
}

pub fn friendly_error(stderr: &str) -> String {
    let text = stderr.to_ascii_lowercase();
    if text.contains("redirect to login")
        || text.contains("authenticationerror")
        || text.contains("401 unauthorized")
    {
        "Instagram 需要有效登入。請在設定中選擇最新匯出的 cookies.txt。".into()
    } else if text.contains("cookie")
        && (text.contains("decrypt")
            || text.contains("could not copy")
            || text.contains("permission"))
    {
        "Chrome／Edge Cookie 解密失敗。請改在設定中選擇由瀏覽器擴充功能匯出的 cookies.txt。".into()
    } else if text.contains("cookie")
        && (text.contains("invalid")
            || text.contains("failed to load")
            || text.contains("does not look like"))
    {
        "無法載入 cookies.txt；請重新匯出 Netscape 格式的 Cookie 檔案。".into()
    } else if text.contains("login required")
        || text.contains("private")
        || text.contains("sign in")
    {
        "這個內容需要登入、Cookie 已失效，或目前帳號沒有存取權。請選擇有效的 cookies.txt。".into()
    } else if text.contains("unsupported url") || text.contains("no suitable extractor") {
        "這個網址或內容類型目前不受支援。".into()
    } else if text.contains("not available")
        || text.contains("video unavailable")
        || text.contains("media is not available")
    {
        "內容不存在、已刪除，或目前無法觀看。".into()
    } else if text.contains("geo") || text.contains("country") || text.contains("region") {
        "這個內容受到地區限制，無法在目前位置下載。".into()
    } else if text.contains("no space left")
        || text.contains("disk full")
        || text.contains("not enough space")
    {
        "磁碟空間不足，請釋放空間或改用其他儲存位置。".into()
    } else if text.contains("ffmpeg") && (text.contains("not found") || text.contains("error")) {
        "FFmpeg 轉檔失敗或工具遺失，請重新安裝 Omni Fetch。".into()
    } else if text.contains("timed out")
        || text.contains("network")
        || text.contains("connection")
        || text.contains("http error")
    {
        "網路連線失敗。請確認連線後重試。".into()
    } else {
        let useful = stderr
            .lines()
            .rev()
            .find(|line| line.to_ascii_lowercase().contains("error:"));
        useful
            .map(|line| line.trim_start_matches("ERROR:").trim().to_string())
            .filter(|line| !line.is_empty())
            .unwrap_or_else(|| "下載工具執行失敗，請稍後重試或更新下載引擎。".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_cookie_failure() {
        assert!(friendly_error("ERROR: could not copy Chrome cookie database").contains("Cookie"));
    }
    #[test]
    fn maps_disk_failure() {
        assert!(friendly_error("No space left on device").contains("磁碟"));
    }
    #[test]
    fn does_not_expose_empty_output() {
        assert!(!friendly_error("").is_empty());
    }

    #[test]
    fn maps_web_access_and_extractor_failures() {
        assert!(
            friendly_error_for_request("HTTP Error 403: Forbidden", Platform::Web, false)
                .contains("來源頁網址")
        );
        assert!(
            friendly_error_for_request("Unsupported URL", Platform::Web, false).contains(".m3u8")
        );
        assert!(
            friendly_error_for_request("This video is DRM protected", Platform::Web, true)
                .contains("DRM")
        );
    }
}
