use crate::{
    errors::friendly_error,
    models::{DownloadRequest, DownloadTask, OutputFormat, Platform, ProgressEvent, TaskStatus},
    progress::{parse_item, parse_output, parse_progress},
    tools,
    validation::{safe_title, validate_cookie_file, validate_media_url, validate_output_directory},
};
use std::{collections::HashMap, path::PathBuf, process::Stdio, sync::Arc};
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Child,
    sync::{mpsc, RwLock},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub tasks: Arc<RwLock<HashMap<String, DownloadTask>>>,
    requests: Arc<RwLock<HashMap<String, DownloadRequest>>>,
    cancellations: Arc<RwLock<HashMap<String, CancellationToken>>>,
    sender: mpsc::Sender<Job>,
}

pub(crate) struct Job {
    id: String,
    request: DownloadRequest,
    token: CancellationToken,
}
enum ProcessLine {
    Stdout(String),
    Stderr(String),
}

pub fn create() -> (AppState, mpsc::Receiver<Job>) {
    let (sender, receiver) = mpsc::channel(256);
    (
        AppState {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            requests: Arc::new(RwLock::new(HashMap::new())),
            cancellations: Arc::new(RwLock::new(HashMap::new())),
            sender,
        },
        receiver,
    )
}

fn new_task(id: String, request: DownloadRequest, platform: Platform) -> DownloadTask {
    DownloadTask {
        id,
        title: safe_title(request.title.as_deref().unwrap_or("未命名媒體")),
        request,
        platform,
        status: TaskStatus::Queued,
        percent: 0.0,
        downloaded_bytes: None,
        total_bytes: None,
        speed: None,
        eta: None,
        current_item: None,
        outputs: Vec::new(),
        error: None,
    }
}

pub async fn enqueue(
    state: &AppState,
    mut request: DownloadRequest,
) -> Result<DownloadTask, String> {
    let (canonical_url, platform) = validate_media_url(&request.url)?;
    if platform != request.platform {
        return Err(match request.platform {
            Platform::Instagram => "目前是 Instagram 模式，請貼上 Instagram 網址。",
            Platform::Youtube => "目前是 YouTube 模式，請貼上 YouTube 網址。",
        }
        .into());
    }
    request.url = canonical_url.to_string();
    validate_output_directory(&request.output_directory)?;
    if request.browser_source == crate::models::BrowserSource::CookiesFile {
        let path = request
            .cookie_file_path
            .as_deref()
            .ok_or("請先選擇 cookies.txt 檔案。")?;
        request.cookie_file_path = Some(validate_cookie_file(path)?.to_string_lossy().to_string());
    }
    if platform == Platform::Instagram && request.output_format != OutputFormat::Original {
        return Err("Instagram 內容第一版僅支援原始格式。".into());
    }
    if let Some(height) = request.quality_height {
        if !(144..=4320).contains(&height) {
            return Err("選擇的影片畫質無效。".into());
        }
    }
    let id = Uuid::new_v4().to_string();
    let token = CancellationToken::new();
    let task = new_task(id.clone(), request.clone(), platform);
    state.tasks.write().await.insert(id.clone(), task.clone());
    state
        .requests
        .write()
        .await
        .insert(id.clone(), request.clone());
    state
        .cancellations
        .write()
        .await
        .insert(id.clone(), token.clone());
    state
        .sender
        .send(Job { id, request, token })
        .await
        .map_err(|_| "下載佇列目前無法使用。")?;
    Ok(task)
}

pub async fn cancel(state: &AppState, id: &str) -> Result<(), String> {
    let token = state
        .cancellations
        .read()
        .await
        .get(id)
        .cloned()
        .ok_or("找不到指定的下載任務。")?;
    token.cancel();
    Ok(())
}

pub async fn retry(state: &AppState, id: &str) -> Result<DownloadTask, String> {
    let task = state
        .tasks
        .read()
        .await
        .get(id)
        .cloned()
        .ok_or("找不到指定的下載任務。")?;
    if !matches!(task.status, TaskStatus::Failed | TaskStatus::Cancelled) {
        return Err("只有失敗或已取消的任務可以重試。".into());
    }
    enqueue(state, task.request).await
}

async fn publish(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    mutate: impl FnOnce(&mut DownloadTask),
) {
    let snapshot = {
        let mut tasks = state.tasks.write().await;
        let Some(task) = tasks.get_mut(id) else {
            return;
        };
        mutate(task);
        task.clone()
    };
    let _ = app.emit("download://state-changed", snapshot);
}

pub fn start_worker(app: AppHandle, state: AppState, mut receiver: mpsc::Receiver<Job>) {
    tauri::async_runtime::spawn(async move {
        while let Some(job) = receiver.recv().await {
            if job.token.is_cancelled() {
                publish(&app, &state, &job.id, |task| {
                    task.status = TaskStatus::Cancelled
                })
                .await;
                continue;
            }
            run_job(&app, &state, job).await;
        }
    });
}

fn ytdlp_download_args(request: &DownloadRequest) -> Result<Vec<String>, String> {
    let output_dir = validate_output_directory(&request.output_directory)?;
    let template: PathBuf = output_dir.join("%(title).120B [%(id)s].%(ext)s");
    let mut args = vec![
        "--newline".into(), "--continue".into(), "--windows-filenames".into(), "--trim-filenames".into(), "180".into(),
        "--progress-template".into(), "download:OF_PROGRESS|%(progress._percent_str)s|%(progress.downloaded_bytes)s|%(progress.total_bytes_estimate)s|%(progress._speed_str)s|%(progress._eta_str)s".into(),
        "--print".into(), "before_dl:OF_ITEM|%(playlist_index)s/%(playlist_count)s".into(),
        "--print".into(), "after_move:OF_FILE|%(filepath)s".into(),
        "--output".into(), template.to_string_lossy().to_string(),
    ];
    args.push("--no-playlist".into());
    match request.output_format {
        OutputFormat::Mp4 => {
            let selector = request
                .quality_height
                .map(|height| format!("bv*[height<={height}]+ba/b[height<={height}]"))
                .unwrap_or_else(|| "bv*+ba/b".into());
            args.extend([
                "--format".into(),
                selector,
                "--merge-output-format".into(),
                "mp4".into(),
            ]);
        }
        OutputFormat::Mp3 => args.extend([
            "--extract-audio".into(),
            "--audio-format".into(),
            "mp3".into(),
            "--audio-quality".into(),
            "0".into(),
        ]),
        OutputFormat::Original => {}
    }
    args.push(request.url.clone());
    Ok(args)
}

fn gallery_download_args(request: &DownloadRequest) -> Result<Vec<String>, String> {
    let output_dir = validate_output_directory(&request.output_directory)?;
    Ok(vec![
        "--config-ignore".into(),
        "--no-input".into(),
        "--no-colors".into(),
        "--windows-filenames".into(),
        "--directory".into(),
        output_dir.to_string_lossy().to_string(),
        "--Print".into(),
        "after:OF_FILE|{_path}".into(),
        request.url.clone(),
    ])
}

async fn run_job(app: &AppHandle, state: &AppState, job: Job) {
    let (_, platform) = match validate_media_url(&job.request.url) {
        Ok(value) => value,
        Err(error) => {
            publish(app, state, &job.id, |task| {
                task.status = TaskStatus::Failed;
                task.error = Some(error);
            })
            .await;
            return;
        }
    };
    publish(app, state, &job.id, |task| {
        task.status = TaskStatus::Analyzing
    })
    .await;
    let command_and_args = match platform {
        Platform::Youtube => tools::ytdlp_command(
            app,
            job.request.browser_source,
            job.request.cookie_file_path.as_deref(),
        )
        .and_then(|command| ytdlp_download_args(&job.request).map(|args| (command, args))),
        Platform::Instagram => tools::gallery_command(
            app,
            job.request.browser_source,
            job.request.cookie_file_path.as_deref(),
        )
        .and_then(|command| gallery_download_args(&job.request).map(|args| (command, args))),
    };
    let (mut command, args) = match command_and_args {
        Ok(value) => value,
        Err(error) => {
            publish(app, state, &job.id, |task| {
                task.status = TaskStatus::Failed;
                task.error = Some(error);
            })
            .await;
            return;
        }
    };
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            publish(app, state, &job.id, |task| {
                task.status = TaskStatus::Failed;
                task.error = Some("無法啟動下載引擎，請執行工具準備腳本或重新安裝。".into());
            })
            .await;
            return;
        }
    };
    let pid = child.id();
    let (line_tx, mut line_rx) = mpsc::channel(128);
    if let Some(stdout) = child.stdout.take() {
        let tx = line_tx.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx.send(ProcessLine::Stdout(line)).await.is_err() {
                    break;
                }
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let tx = line_tx.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx.send(ProcessLine::Stderr(line)).await.is_err() {
                    break;
                }
            }
        });
    }
    drop(line_tx);
    publish(app, state, &job.id, |task| {
        task.status = TaskStatus::Downloading
    })
    .await;
    let mut stderr_log = String::new();
    let mut cancelled = false;
    loop {
        tokio::select! {
            _ = job.token.cancelled() => { cancelled = true; terminate_tree(&mut child, pid).await; break; }
            line = line_rx.recv() => match line {
                Some(ProcessLine::Stdout(line)) => {
                    if let Some(progress) = parse_progress(&line) {
                        let event = ProgressEvent { task_id: job.id.clone(), percent: progress.percent, downloaded_bytes: progress.downloaded_bytes, total_bytes: progress.total_bytes, speed: progress.speed.clone(), eta: progress.eta.clone(), current_item: None };
                        publish(app, state, &job.id, |task| { task.percent = progress.percent; task.downloaded_bytes = progress.downloaded_bytes; task.total_bytes = progress.total_bytes; task.speed = progress.speed; task.eta = progress.eta; }).await;
                        let _ = app.emit("download://progress", event);
                    } else if let Some(path) = parse_output(&line) {
                        let expected = job.request.item_count.unwrap_or(1).max(1);
                        publish(app, state, &job.id, |task| {
                            if !task.outputs.contains(&path) { task.outputs.push(path); }
                            if platform == Platform::Instagram {
                                task.percent = (task.outputs.len() as f64 / expected as f64 * 100.0).min(99.0);
                            }
                        }).await;
                    } else if let Some(item) = parse_item(&line) {
                        publish(app, state, &job.id, |task| task.current_item = Some(item)).await;
                    }
                }
                Some(ProcessLine::Stderr(line)) => {
                    if line.contains("[ExtractAudio]") || line.contains("[Merger]") || line.contains("[VideoConvertor]") {
                        publish(app, state, &job.id, |task| task.status = TaskStatus::Converting).await;
                    }
                    if stderr_log.len() < 65_536 { stderr_log.push_str(&line); stderr_log.push('\n'); }
                }
                None => break,
            }
        }
    }
    let status = child.wait().await.ok();
    if cancelled || job.token.is_cancelled() {
        publish(app, state, &job.id, |task| {
            task.status = TaskStatus::Cancelled;
            task.speed = None;
            task.eta = None;
        })
        .await;
    } else if status.is_some_and(|s| s.success()) {
        publish(app, state, &job.id, |task| {
            task.status = TaskStatus::Completed;
            task.percent = 100.0;
            task.speed = None;
            task.eta = None;
        })
        .await;
    } else {
        let error = friendly_error(&stderr_log);
        publish(app, state, &job.id, |task| {
            task.status = TaskStatus::Failed;
            task.error = Some(error);
            task.speed = None;
            task.eta = None;
        })
        .await;
    }
    state.cancellations.write().await.remove(&job.id);
}

async fn terminate_tree(child: &mut Child, pid: Option<u32>) {
    #[cfg(windows)]
    if let Some(pid) = pid {
        let mut command = tokio::process::Command::new("taskkill");
        tools::hide_window(&mut command);
        let _ = command
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output()
            .await;
    }
    let _ = child.kill().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(format: OutputFormat) -> DownloadRequest {
        DownloadRequest {
            url: "https://youtu.be/test".into(),
            platform: Platform::Youtube,
            browser_source: Default::default(),
            cookie_file_path: None,
            output_format: format,
            quality_height: Some(1080),
            output_directory: std::env::temp_dir().display().to_string(),
            title: None,
            item_count: Some(1),
        }
    }
    #[test]
    fn builds_mp4_arguments_without_shell_text() {
        let args = ytdlp_download_args(&request(OutputFormat::Mp4)).unwrap();
        assert!(args.contains(&"bv*[height<=1080]+ba/b[height<=1080]".to_string()));
        assert_eq!(args.last().unwrap(), "https://youtu.be/test");
    }
    #[test]
    fn builds_mp3_arguments() {
        let args = ytdlp_download_args(&request(OutputFormat::Mp3)).unwrap();
        assert!(args.contains(&"--extract-audio".to_string()));
        assert!(args.contains(&"--audio-quality".to_string()));
    }
    #[test]
    fn builds_instagram_gallery_arguments() {
        let mut value = request(OutputFormat::Original);
        value.url = "https://www.instagram.com/p/demo/".into();
        value.platform = Platform::Instagram;
        let args = gallery_download_args(&value).unwrap();
        assert!(args.contains(&"--directory".to_string()));
        assert!(args.contains(&"after:OF_FILE|{_path}".to_string()));
    }
}
