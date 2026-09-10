mod analysis;
mod errors;
mod models;
mod progress;
mod queue;
mod settings;
mod tools;
mod validation;

use models::{
    AnalyzeRequest, AppSettings, DownloadRequest, DownloadTask, EngineInfo, EngineUpdateInfo,
    MediaAnalysis, TaskStatus,
};
use queue::AppState;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|_| "無法取得應用程式設定路徑。")?
        .join("settings.json"))
}

#[tauri::command]
async fn analyze_url(app: AppHandle, request: AnalyzeRequest) -> Result<MediaAnalysis, String> {
    analysis::analyze(&app, request).await
}

#[tauri::command]
async fn enqueue_download(
    state: State<'_, AppState>,
    request: DownloadRequest,
) -> Result<DownloadTask, String> {
    queue::enqueue(&state, request).await
}

#[tauri::command]
async fn cancel_task(state: State<'_, AppState>, task_id: String) -> Result<(), String> {
    queue::cancel(&state, &task_id).await
}

#[tauri::command]
async fn retry_task(state: State<'_, AppState>, task_id: String) -> Result<DownloadTask, String> {
    queue::retry(&state, &task_id).await
}

#[tauri::command]
async fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    Ok(settings::load(&settings_path(&app)?))
}

#[tauri::command]
async fn save_settings(app: AppHandle, mut settings: AppSettings) -> Result<AppSettings, String> {
    crate::validation::validate_output_directory(&settings.output_directory)?;
    if settings.browser_source == models::BrowserSource::CookiesFile {
        let path = settings
            .cookie_file_path
            .as_deref()
            .ok_or("請先選擇 cookies.txt 檔案。")?;
        settings.cookie_file_path = Some(
            crate::validation::validate_cookie_file(path)?
                .to_string_lossy()
                .to_string(),
        );
    }
    crate::settings::save(&settings_path(&app)?, &settings)?;
    Ok(settings)
}

#[tauri::command]
async fn get_engine_info(app: AppHandle) -> Result<EngineInfo, String> {
    tools::engine_info(&app).await
}

#[tauri::command]
async fn check_engine_update(app: AppHandle) -> Result<EngineUpdateInfo, String> {
    tools::check_update(&app).await
}

#[tauri::command]
async fn install_engine_update(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<EngineInfo, String> {
    let active = state.tasks.read().await.values().any(|task| {
        matches!(
            task.status,
            TaskStatus::Analyzing | TaskStatus::Downloading | TaskStatus::Converting
        )
    });
    if active {
        return Err("請等待目前的下載與轉檔任務完成後再更新引擎。".into());
    }
    let _ = app.emit(
        "engine://update-state",
        serde_json::json!({ "status": "downloading" }),
    );
    let result = tools::install_update(&app).await;
    let payload = match &result {
        Ok(info) => serde_json::json!({ "status": "completed", "version": info.yt_dlp_version }),
        Err(message) => serde_json::json!({ "status": "failed", "message": message }),
    };
    let _ = app.emit("engine://update-state", payload);
    result
}

#[tauri::command]
async fn open_output_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let requested = Path::new(&path);
    let folder = if requested.is_file() {
        requested.parent().unwrap_or(requested)
    } else {
        requested
    };
    let canonical = folder.canonicalize().map_err(|_| "輸出資料夾不存在。")?;
    let task_dirs: Vec<_> = state
        .tasks
        .read()
        .await
        .values()
        .map(|task| PathBuf::from(&task.request.output_directory))
        .collect();
    let allowed = task_dirs
        .iter()
        .filter_map(|dir| dir.canonicalize().ok())
        .any(|dir| canonical.starts_with(dir));
    if !allowed {
        return Err("只能開啟目前下載任務的輸出資料夾。".into());
    }
    app.opener()
        .open_path(canonical.to_string_lossy().to_string(), None::<&str>)
        .map_err(|_| "無法開啟輸出資料夾。".into())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (state, receiver) = queue::create();
    let worker_state = state.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .setup(move |app| {
            queue::start_worker(app.handle().clone(), worker_state, receiver);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            analyze_url,
            enqueue_download,
            cancel_task,
            retry_task,
            get_settings,
            save_settings,
            get_engine_info,
            check_engine_update,
            install_engine_update,
            open_output_folder,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Omni Fetch");
}
