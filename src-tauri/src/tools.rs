use crate::{
    models::{BrowserSource, EngineInfo, EngineUpdateInfo},
    validation::validate_cookie_file,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
use tauri::{AppHandle, Manager};
use tokio::{fs, process::Command};

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
const DOWNLOAD_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const SUMS_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/SHA2-256SUMS";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

pub struct ToolPaths {
    pub ytdlp: PathBuf,
    pub gallerydl: PathBuf,
    pub ffmpeg_dir: Option<PathBuf>,
    pub source: String,
}

fn binary_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    }
}

fn first_existing(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|path| path.is_file())
}

pub fn paths(app: &AppHandle) -> Result<ToolPaths, String> {
    let local = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "無法取得 App Data 路徑。")?;
    let updated = local.join("engine").join(binary_name("yt-dlp"));
    let executable_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let resource_dir = app.path().resource_dir().ok();
    let dev_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
    let target = "x86_64-pc-windows-msvc";
    let bundled_name = if cfg!(windows) {
        format!("yt-dlp-{target}.exe")
    } else {
        format!("yt-dlp-{target}")
    };
    let bundled = first_existing(
        executable_dir
            .iter()
            .chain(resource_dir.iter())
            .map(|dir| dir.join(binary_name("yt-dlp")))
            .chain([
                dev_dir.join(binary_name("yt-dlp")),
                dev_dir.join(&bundled_name),
            ]),
    );
    let (ytdlp, source) = if updated.is_file() {
        (updated, "updated")
    } else if let Some(path) = bundled {
        (path, "bundled")
    } else {
        (PathBuf::from(binary_name("yt-dlp")), "system")
    };

    let gallery_bundled_name = if cfg!(windows) {
        format!("gallery-dl-{target}.exe")
    } else {
        format!("gallery-dl-{target}")
    };
    let gallerydl = first_existing(
        executable_dir
            .iter()
            .chain(resource_dir.iter())
            .map(|dir| dir.join(binary_name("gallery-dl")))
            .chain([
                dev_dir.join(binary_name("gallery-dl")),
                dev_dir.join(gallery_bundled_name),
                PathBuf::from(binary_name("gallery-dl")),
            ]),
    )
    .unwrap_or_else(|| PathBuf::from(binary_name("gallery-dl")));

    let ffmpeg = first_existing(
        executable_dir
            .iter()
            .chain(resource_dir.iter())
            .map(|dir| dir.join(binary_name("ffmpeg")))
            .chain([
                dev_dir.join(binary_name("ffmpeg")),
                dev_dir.join(format!("ffmpeg-{target}.exe")),
                PathBuf::from(binary_name("ffmpeg")),
            ]),
    );
    Ok(ToolPaths {
        ytdlp,
        gallerydl,
        ffmpeg_dir: ffmpeg.and_then(|p| p.parent().map(Path::to_path_buf)),
        source: source.into(),
    })
}

pub fn gallery_command(
    app: &AppHandle,
    browser: BrowserSource,
    cookie_file_path: Option<&str>,
) -> Result<Command, String> {
    let tools = paths(app)?;
    let mut command = Command::new(&tools.gallerydl);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .creation_flags_no_window()
        .args(authentication_args(browser, cookie_file_path)?);
    Ok(command)
}

fn authentication_args(
    browser: BrowserSource,
    cookie_file_path: Option<&str>,
) -> Result<Vec<String>, String> {
    match browser {
        BrowserSource::Chrome => Ok(vec!["--cookies-from-browser".into(), "chrome".into()]),
        BrowserSource::Edge => Ok(vec!["--cookies-from-browser".into(), "edge".into()]),
        BrowserSource::CookiesFile => {
            let path = cookie_file_path.ok_or("請先選擇 cookies.txt 檔案。")?;
            let canonical = validate_cookie_file(path)?;
            Ok(vec![
                "--cookies".into(),
                canonical.to_string_lossy().to_string(),
            ])
        }
        BrowserSource::None => Ok(Vec::new()),
    }
}

pub fn ytdlp_command(
    app: &AppHandle,
    browser: BrowserSource,
    cookie_file_path: Option<&str>,
) -> Result<Command, String> {
    let tools = paths(app)?;
    let mut command = Command::new(&tools.ytdlp);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .creation_flags_no_window();
    if let Some(dir) = tools.ffmpeg_dir {
        command.arg("--ffmpeg-location").arg(dir);
    }
    command.args(authentication_args(browser, cookie_file_path)?);
    Ok(command)
}

async fn command_version(path: &Path, arg: &str) -> String {
    Command::new(path)
        .arg(arg)
        .creation_flags_no_window()
        .output()
        .await
        .ok()
        .filter(|out| out.status.success())
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("未知")
                .trim()
                .to_string()
        })
        .unwrap_or_else(|| "未安裝".into())
}

pub async fn engine_info(app: &AppHandle) -> Result<EngineInfo, String> {
    let tools = paths(app)?;
    let yt_dlp_version = command_version(&tools.ytdlp, "--version").await;
    let gallery_dl_version = command_version(&tools.gallerydl, "--version").await;
    let ffmpeg_path = tools
        .ffmpeg_dir
        .as_ref()
        .map(|dir| dir.join(binary_name("ffmpeg")))
        .unwrap_or_else(|| PathBuf::from(binary_name("ffmpeg")));
    let ffmpeg_version = command_version(&ffmpeg_path, "-version")
        .await
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    Ok(EngineInfo {
        yt_dlp_version,
        gallery_dl_version,
        ffmpeg_version,
        source: tools.source,
    })
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("Omni-Fetch/0.1")
        .build()
        .map_err(|_| "無法建立安全網路連線。".into())
}

pub async fn check_update(app: &AppHandle) -> Result<EngineUpdateInfo, String> {
    let current = engine_info(app).await?.yt_dlp_version;
    let release: Release = client()?
        .get(LATEST_RELEASE_API)
        .send()
        .await
        .map_err(|_| "無法連線至 GitHub 檢查更新。")?
        .error_for_status()
        .map_err(|_| "GitHub 暫時無法提供更新資訊。")?
        .json()
        .await
        .map_err(|_| "更新資訊格式不正確。")?;
    let latest = release.tag_name.trim_start_matches('v').to_string();
    Ok(EngineUpdateInfo {
        update_available: current != latest,
        current_version: current,
        latest_version: latest,
    })
}

pub async fn install_update(app: &AppHandle) -> Result<EngineInfo, String> {
    let http = client()?;
    let (binary, sums) = tokio::try_join!(
        async {
            http.get(DOWNLOAD_URL)
                .send()
                .await
                .map_err(|_| "下載 yt-dlp 更新失敗。")?
                .error_for_status()
                .map_err(|_| "找不到 yt-dlp 更新檔。")?
                .bytes()
                .await
                .map_err(|_| "讀取 yt-dlp 更新失敗。")
        },
        async {
            http.get(SUMS_URL)
                .send()
                .await
                .map_err(|_| "下載更新校驗碼失敗。")?
                .error_for_status()
                .map_err(|_| "找不到更新校驗碼。")?
                .text()
                .await
                .map_err(|_| "讀取更新校驗碼失敗。")
        }
    )?;
    let expected = sums
        .lines()
        .find(|line| line.trim_end().ends_with("yt-dlp.exe"))
        .and_then(|line| line.split_whitespace().next())
        .ok_or("官方校驗清單中找不到 yt-dlp.exe。")?;
    let actual = hex::encode(Sha256::digest(&binary));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err("下載引擎校驗失敗，已拒絕安裝。".into());
    }

    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "無法取得 App Data 路徑。")?
        .join("engine");
    fs::create_dir_all(&dir)
        .await
        .map_err(|_| "無法建立下載引擎資料夾。")?;
    let destination = dir.join(binary_name("yt-dlp"));
    let temporary = dir.join("yt-dlp.download.exe");
    fs::write(&temporary, &binary)
        .await
        .map_err(|_| "無法寫入下載引擎更新。")?;
    let version = command_version(&temporary, "--version").await;
    if version == "未安裝" {
        let _ = fs::remove_file(&temporary).await;
        return Err("新版下載引擎健康檢查失敗。".into());
    }
    let backup = dir.join("yt-dlp.previous.exe");
    if backup.exists() {
        let _ = fs::remove_file(&backup).await;
    }
    if destination.exists() {
        fs::rename(&destination, &backup)
            .await
            .map_err(|_| "下載引擎正在使用中，請等待任務完成後再更新。")?;
    }
    if fs::rename(&temporary, &destination).await.is_err() {
        if backup.exists() {
            let _ = fs::rename(&backup, &destination).await;
        }
        return Err("無法啟用新版下載引擎，已回復原版本。".into());
    }
    if backup.exists() {
        let _ = fs::remove_file(&backup).await;
    }
    engine_info(app).await
}

trait WindowsCommandExt {
    fn creation_flags_no_window(&mut self) -> &mut Self;
}

pub(crate) fn hide_window(command: &mut Command) {
    command.creation_flags_no_window();
}
impl WindowsCommandExt for Command {
    fn creation_flags_no_window(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            self.creation_flags(0x08000000);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_browser_authentication_arguments() {
        assert_eq!(
            authentication_args(BrowserSource::Chrome, None).unwrap(),
            ["--cookies-from-browser", "chrome"]
        );
        assert!(authentication_args(BrowserSource::CookiesFile, None).is_err());
    }

    #[test]
    fn builds_cookie_file_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("instagram cookies.txt");
        std::fs::write(
            &path,
            "# Netscape HTTP Cookie File\n.instagram.com\tTRUE\t/\tTRUE\t0\tsessionid\tsecret\n",
        )
        .unwrap();
        let args = authentication_args(BrowserSource::CookiesFile, path.to_str()).unwrap();
        assert_eq!(args[0], "--cookies");
        assert_eq!(Path::new(&args[1]), path.canonicalize().unwrap());
    }
}
