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
}
