use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Youtube,
    Instagram,
}

impl Default for Platform {
    fn default() -> Self {
        Self::Instagram
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum BrowserSource {
    #[default]
    None,
    Chrome,
    Edge,
    CookiesFile,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Mp4,
    Mp3,
    Original,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Queued,
    Analyzing,
    Downloading,
    Converting,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeRequest {
    pub url: String,
    pub platform: Platform,
    pub browser_source: BrowserSource,
    pub cookie_file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityOption {
    pub height: u32,
    pub label: String,
    pub estimated_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: String,
    pub title: String,
    pub media_type: String,
    pub thumbnail: Option<String>,
    pub duration_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAnalysis {
    pub url: String,
    pub platform: Platform,
    pub title: String,
    pub thumbnail: Option<String>,
    pub uploader: Option<String>,
    pub duration_seconds: Option<f64>,
    pub is_carousel: bool,
    pub items: Vec<MediaItem>,
    pub qualities: Vec<QualityOption>,
    pub supported_outputs: Vec<OutputFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRequest {
    pub url: String,
    pub platform: Platform,
    pub browser_source: BrowserSource,
    pub cookie_file_path: Option<String>,
    pub output_format: OutputFormat,
    pub quality_height: Option<u32>,
    pub output_directory: String,
    pub title: Option<String>,
    pub item_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: String,
    pub request: DownloadRequest,
    pub title: String,
    pub platform: Platform,
    pub status: TaskStatus,
    pub percent: f64,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub current_item: Option<String>,
    pub outputs: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub output_directory: String,
    #[serde(default)]
    pub platform_mode: Platform,
    pub browser_source: BrowserSource,
    pub cookie_file_path: Option<String>,
    pub notice_accepted: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub yt_dlp_version: String,
    pub gallery_dl_version: String,
    pub ffmpeg_version: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub task_id: String,
    pub percent: f64,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub current_item: Option<String>,
}
