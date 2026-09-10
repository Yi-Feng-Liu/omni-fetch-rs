use crate::models::AppSettings;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn default_settings() -> AppSettings {
    AppSettings {
        output_directory: dirs::download_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .to_string_lossy()
            .to_string(),
        platform_mode: Default::default(),
        browser_source: Default::default(),
        cookie_file_path: None,
        notice_accepted: false,
    }
}

pub fn load(path: &Path) -> AppSettings {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(default_settings)
}

pub fn save(path: &Path, settings: &AppSettings) -> Result<(), String> {
    if settings.output_directory.is_empty() || !Path::new(&settings.output_directory).is_dir() {
        return Err("預設儲存資料夾不存在或無法存取。".into());
    }
    let parent = path.parent().ok_or("設定檔路徑無效。")?;
    fs::create_dir_all(parent).map_err(|_| "無法建立設定資料夾。")?;
    let temp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(settings).map_err(|_| "無法序列化設定。")?;
    fs::write(&temp, data).map_err(|_| "無法寫入設定檔。")?;
    fs::rename(temp, path).map_err(|_| "無法儲存設定檔。".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trips_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let value = AppSettings {
            output_directory: dir.path().display().to_string(),
            platform_mode: Default::default(),
            browser_source: Default::default(),
            cookie_file_path: None,
            notice_accepted: true,
        };
        save(&path, &value).unwrap();
        assert!(load(&path).notice_accepted);
    }
}
