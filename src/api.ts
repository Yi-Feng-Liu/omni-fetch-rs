import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AnalyzeRequest, AppSettings, DownloadRequest, DownloadTask, EngineInfo,
  EngineUpdateInfo, MediaAnalysis, ProgressEvent,
} from "./types";

export const api = {
  analyze: (request: AnalyzeRequest) => invoke<MediaAnalysis>("analyze_url", { request }),
  enqueue: (request: DownloadRequest) => invoke<DownloadTask>("enqueue_download", { request }),
  cancel: (taskId: string) => invoke<void>("cancel_task", { taskId }),
  retry: (taskId: string) => invoke<DownloadTask>("retry_task", { taskId }),
  settings: () => invoke<AppSettings>("get_settings"),
  saveSettings: (settings: AppSettings) => invoke<AppSettings>("save_settings", { settings }),
  engineInfo: () => invoke<EngineInfo>("get_engine_info"),
  checkUpdate: () => invoke<EngineUpdateInfo>("check_engine_update"),
  installUpdate: () => invoke<EngineInfo>("install_engine_update"),
  openFolder: (path: string) => invoke<void>("open_output_folder", { path }),
  onProgress: (handler: (event: ProgressEvent) => void): Promise<UnlistenFn> =>
    listen<ProgressEvent>("download://progress", ({ payload }) => handler(payload)),
  onTaskChanged: (handler: (task: DownloadTask) => void): Promise<UnlistenFn> =>
    listen<DownloadTask>("download://state-changed", ({ payload }) => handler(payload)),
};

