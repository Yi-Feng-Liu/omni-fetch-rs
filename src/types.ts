export type Platform = "youtube" | "instagram" | "web";
export type BrowserSource = "none" | "chrome" | "edge" | "cookiesFile";
export type OutputFormat = "mp4" | "mp3" | "original";
export type TaskStatus =
  | "queued" | "analyzing" | "downloading" | "converting"
  | "completed" | "failed" | "cancelled";

export interface AnalyzeRequest {
  url: string;
  platform: Platform;
  sourcePageUrl?: string;
  browserSource: BrowserSource;
  cookieFilePath?: string;
}

export interface QualityOption {
  height: number;
  label: string;
  estimatedBytes?: number;
}

export interface MediaItem {
  id: string;
  title: string;
  mediaType: "image" | "video" | "audio";
  thumbnail?: string;
  durationSeconds?: number;
}

export interface MediaAnalysis {
  url: string;
  platform: Platform;
  title: string;
  thumbnail?: string;
  uploader?: string;
  durationSeconds?: number;
  isCarousel: boolean;
  items: MediaItem[];
  qualities: QualityOption[];
  supportedOutputs: OutputFormat[];
}

export interface DownloadRequest {
  url: string;
  platform: Platform;
  sourcePageUrl?: string;
  outputFilename?: string;
  browserSource: BrowserSource;
  cookieFilePath?: string;
  outputFormat: OutputFormat;
  qualityHeight?: number;
  outputDirectory: string;
  title?: string;
  itemCount?: number;
}

export interface DownloadTask {
  id: string;
  request: DownloadRequest;
  title: string;
  platform: Platform;
  status: TaskStatus;
  percent: number;
  downloadedBytes?: number;
  totalBytes?: number;
  speed?: string;
  eta?: string;
  currentItem?: string;
  outputs: string[];
  error?: string;
}

export interface AppSettings {
  outputDirectory: string;
  platformMode: Platform;
  browserSource: BrowserSource;
  cookieFilePath?: string;
  noticeAccepted: boolean;
}

export interface EngineInfo {
  ytDlpVersion: string;
  galleryDlVersion: string;
  ffmpegVersion: string;
  source: "bundled" | "updated" | "system";
}

export interface EngineUpdateInfo {
  currentVersion: string;
  latestVersion: string;
  updateAvailable: boolean;
}

export interface ProgressEvent {
  taskId: string;
  percent: number;
  downloadedBytes?: number;
  totalBytes?: number;
  speed?: string;
  eta?: string;
  currentItem?: string;
}
