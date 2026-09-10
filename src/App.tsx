import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  CheckCircle2, CircleAlert, Download, ExternalLink, FolderOpen, Instagram,
  LoaderCircle, Music2, RefreshCw, RotateCcw, Settings2, ShieldCheck, Square,
  Video, Youtube,
} from "lucide-react";
import { api } from "./api";
import type {
  AppSettings, BrowserSource, DownloadTask, EngineInfo, MediaAnalysis, OutputFormat,
  ProgressEvent,
} from "./types";

const initialSettings: AppSettings = {
  outputDirectory: "",
  platformMode: "instagram",
  browserSource: "none",
  cookieFilePath: undefined,
  noticeAccepted: true,
};

const statusText: Record<DownloadTask["status"], string> = {
  queued: "等待中", analyzing: "分析中", downloading: "下載中", converting: "轉檔中",
  completed: "已完成", failed: "失敗", cancelled: "已取消",
};

function errorText(error: unknown) {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "發生未預期的錯誤，請稍後再試。";
}

function bytes(value?: number) {
  if (!value) return "—";
  const units = ["B", "KB", "MB", "GB"];
  let amount = value;
  let index = 0;
  while (amount >= 1024 && index < units.length - 1) { amount /= 1024; index += 1; }
  return `${amount.toFixed(index > 1 ? 1 : 0)} ${units[index]}`;
}

function taskIsActive(task: DownloadTask) {
  return ["queued", "analyzing", "downloading", "converting"].includes(task.status);
}

export default function App() {
  const [url, setUrl] = useState("");
  const [settings, setSettings] = useState<AppSettings>(initialSettings);
  const [analysis, setAnalysis] = useState<MediaAnalysis>();
  const [format, setFormat] = useState<OutputFormat>("mp4");
  const [quality, setQuality] = useState<number>();
  const [tasks, setTasks] = useState<DownloadTask[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [showSettings, setShowSettings] = useState(false);
  const [showNotice, setShowNotice] = useState(false);
  const [engine, setEngine] = useState<EngineInfo>();
  const [updating, setUpdating] = useState(false);

  useEffect(() => {
    Promise.all([api.settings(), api.engineInfo()])
      .then(([saved, info]) => {
        setSettings(saved);
        setShowNotice(!saved.noticeAccepted);
        setEngine(info);
      })
      .catch((error) => setMessage(errorText(error)));

    const unlisteners: Promise<() => void>[] = [
      api.onTaskChanged((task) => setTasks((current) => {
        const exists = current.some((item) => item.id === task.id);
        return exists ? current.map((item) => item.id === task.id ? task : item) : [task, ...current];
      })),
      api.onProgress((event) => updateProgress(event)),
    ];
    return () => { unlisteners.forEach((pending) => void pending.then((fn) => fn())); };
  }, []);

  const selectedQuality = useMemo(
    () => quality ?? analysis?.qualities.at(-1)?.height,
    [analysis, quality],
  );

  function updateProgress(event: ProgressEvent) {
    setTasks((current) => current.map((task) => task.id === event.taskId ? {
      ...task,
      percent: event.percent,
      downloadedBytes: event.downloadedBytes,
      totalBytes: event.totalBytes,
      speed: event.speed,
      eta: event.eta,
      currentItem: event.currentItem,
    } : task));
  }

  async function analyze() {
    if (!url.trim()) return;
    setBusy(true); setMessage(""); setAnalysis(undefined);
    try {
      const result = await api.analyze({
        url: url.trim(),
        platform: settings.platformMode,
        browserSource: settings.browserSource,
        cookieFilePath: settings.cookieFilePath,
      });
      setAnalysis(result);
      const defaultFormat: OutputFormat = result.platform === "instagram" ? "original" : "mp4";
      setFormat(defaultFormat);
      setQuality(result.qualities.at(-1)?.height);
    } catch (error) { setMessage(errorText(error)); }
    finally { setBusy(false); }
  }

  async function enqueue() {
    if (!analysis || !settings.outputDirectory) return;
    setBusy(true); setMessage("");
    try {
      const task = await api.enqueue({
        url: analysis.url,
        platform: analysis.platform,
        browserSource: settings.browserSource,
        cookieFilePath: settings.cookieFilePath,
        outputFormat: format,
        qualityHeight: format === "mp4" ? selectedQuality : undefined,
        outputDirectory: settings.outputDirectory,
        title: analysis.title,
        itemCount: analysis.items.length,
      });
      setTasks((current) => [task, ...current.filter((item) => item.id !== task.id)]);
      setAnalysis(undefined); setUrl("");
    } catch (error) { setMessage(errorText(error)); }
    finally { setBusy(false); }
  }

  async function chooseDirectory() {
    const selected = await open({ directory: true, multiple: false, defaultPath: settings.outputDirectory });
    if (typeof selected === "string") {
      const next = { ...settings, outputDirectory: selected };
      await persist(next);
    }
  }

  async function chooseCookieFile() {
    const selected = await open({
      directory: false,
      multiple: false,
      defaultPath: settings.cookieFilePath,
      filters: [{ name: "Netscape cookies.txt", extensions: ["txt"] }],
    });
    if (typeof selected === "string") {
      await persist({ ...settings, browserSource: "cookiesFile", cookieFilePath: selected });
    }
  }

  async function changeBrowserSource(source: BrowserSource) {
    if (source === "cookiesFile") {
      await chooseCookieFile();
      return;
    }
    await persist({ ...settings, browserSource: source });
  }

  async function changePlatformMode(platformMode: AppSettings["platformMode"]) {
    setAnalysis(undefined);
    setMessage("");
    await persist({ ...settings, platformMode });
  }

  async function persist(next: AppSettings) {
    const previous = settings;
    setSettings(next);
    try { setSettings(await api.saveSettings(next)); }
    catch (error) { setSettings(previous); setMessage(errorText(error)); }
  }

  async function acceptNotice() {
    await persist({ ...settings, noticeAccepted: true });
    setShowNotice(false);
  }

  async function updateEngine() {
    setUpdating(true); setMessage("");
    try {
      const update = await api.checkUpdate();
      if (!update.updateAvailable) setMessage(`下載引擎已是最新版（${update.currentVersion}）。`);
      else {
        setMessage(`正在安裝 yt-dlp ${update.latestVersion}…`);
        const info = await api.installUpdate();
        setEngine(info); setMessage(`下載引擎已更新至 ${info.ytDlpVersion}。`);
      }
    } catch (error) { setMessage(errorText(error)); }
    finally { setUpdating(false); }
  }

  return (
    <div className="app-shell">
      <header>
        <div className="brand"><div className="brand-mark"><Download size={22} /></div><div><b>Omni Fetch</b><span>MEDIA DOWNLOADER</span></div></div>
        <button className="icon-button" aria-label="設定" onClick={() => setShowSettings(true)}><Settings2 /></button>
      </header>

      <main>
        <section className="hero">
          <div className="mode-switch" aria-label="下載平台">
            <button className={settings.platformMode === "instagram" ? "active" : ""} onClick={() => void changePlatformMode("instagram")}><Instagram size={17} />Instagram</button>
            <button className={settings.platformMode === "youtube" ? "active" : ""} onClick={() => void changePlatformMode("youtube")}><Youtube size={18} />YouTube</button>
          </div>
          <div className="eyebrow">{settings.platformMode === "instagram" ? "PHOTOS · CAROUSELS · REELS" : "VIDEOS · SHORTS · MP3"}</div>
          <h1>貼上連結，<em>帶走你要的內容。</em></h1>
          <p>下載圖片、影片或轉成高品質 MP3。簡單、快速，檔案留在你的電腦。</p>
          <div className="url-box">
            <input aria-label="媒體網址" value={url} onChange={(e) => setUrl(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") void analyze(); }}
              placeholder={settings.platformMode === "instagram" ? "貼上 Instagram 貼文或 Reels 網址…" : "貼上 YouTube 影片或 Shorts 網址…"} />
            <button className="primary" disabled={busy || !url.trim()} onClick={() => void analyze()}>
              {busy ? <LoaderCircle className="spin" size={19} /> : <Download size={19} />} 分析連結
            </button>
          </div>
          <div className="trust"><ShieldCheck size={15} /> 僅使用你選擇的登入來源；Omni Fetch 不保存帳號密碼或 Cookie 內容。</div>
        </section>

        {message && <div className="message" role="status"><CircleAlert size={18} />{message}<button onClick={() => setMessage("")}>×</button></div>}

        {analysis && <section className="analysis card">
          {analysis.thumbnail ? <img src={analysis.thumbnail} alt="媒體縮圖" /> : <div className="thumb-placeholder"><Video /></div>}
          <div className="analysis-body">
            <div className="platform">{analysis.platform === "youtube" ? "YOUTUBE" : "INSTAGRAM"}{analysis.isCarousel && ` · ${analysis.items.length} 個項目`}</div>
            <h2>{analysis.title}</h2>
            {analysis.uploader && <p>{analysis.uploader}</p>}
            <div className="options">
              <label>輸出格式<select value={format} onChange={(e) => setFormat(e.target.value as OutputFormat)}>
                {analysis.supportedOutputs.map((value) => <option key={value} value={value}>{value === "original" ? "原始格式" : value.toUpperCase()}</option>)}
              </select></label>
              {format === "mp4" && analysis.qualities.length > 0 && <label>影片畫質<select value={selectedQuality} onChange={(e) => setQuality(Number(e.target.value))}>
                {analysis.qualities.map((item) => <option value={item.height} key={item.height}>{item.label}{item.estimatedBytes ? ` · ${bytes(item.estimatedBytes)}` : ""}</option>)}
              </select></label>}
              <label className="folder-label">儲存位置<button className="folder-picker" onClick={() => void chooseDirectory()}><FolderOpen size={17} /><span>{settings.outputDirectory || "選擇資料夾"}</span></button></label>
            </div>
            <button className="primary download-button" disabled={busy || !settings.outputDirectory} onClick={() => void enqueue()}><Download size={18} />加入下載佇列</button>
          </div>
        </section>}

        <section className="queue-section">
          <div className="section-heading"><div><span>DOWNLOAD QUEUE</span><h2>下載佇列</h2></div>{tasks.length > 0 && <small>{tasks.filter(taskIsActive).length} 個進行中</small>}</div>
          {tasks.length === 0 ? <div className="empty card"><div><Music2 /></div><h3>還沒有下載項目</h3><p>在上方貼上連結，分析後即可開始。</p></div> :
            <div className="task-list">{tasks.map((task) => <article className="task card" key={task.id}>
              <div className={`task-icon ${task.status}`}>{task.status === "completed" ? <CheckCircle2 /> : task.request.outputFormat === "mp3" ? <Music2 /> : <Video />}</div>
              <div className="task-content">
                <div className="task-title"><strong>{task.title}</strong><span className={task.status}>{statusText[task.status]}</span></div>
                <div className="task-meta">{task.request.outputFormat.toUpperCase()} · {bytes(task.downloadedBytes)}{task.totalBytes ? ` / ${bytes(task.totalBytes)}` : ""}{task.speed ? ` · ${task.speed}` : ""}{task.eta ? ` · 剩餘 ${task.eta}` : ""}</div>
                {task.error && <div className="task-error">{task.error}</div>}
                {taskIsActive(task) && <div className="progress"><i style={{ width: `${Math.max(2, task.percent)}%` }} /></div>}
                {task.outputs.length > 0 && <div className="outputs">{task.outputs.map((item) => <button key={item} onClick={() => void api.openFolder(item)}><ExternalLink size={13} />{item.split(/[\\/]/).at(-1)}</button>)}</div>}
              </div>
              <div className="task-actions">
                {taskIsActive(task) && <button title="取消" onClick={() => void api.cancel(task.id)}><Square size={16} /></button>}
                {(task.status === "failed" || task.status === "cancelled") && <button title="重試" onClick={() => void api.retry(task.id)}><RotateCcw size={16} /></button>}
                {task.status === "completed" && <button title="開啟資料夾" onClick={() => void api.openFolder(task.request.outputDirectory)}><FolderOpen size={16} /></button>}
              </div>
            </article>)}</div>}
        </section>
      </main>

      {showSettings && <div className="overlay" onMouseDown={() => setShowSettings(false)}><aside className="modal" onMouseDown={(e) => e.stopPropagation()}>
        <div className="modal-title"><div><span>SETTINGS</span><h2>設定</h2></div><button onClick={() => setShowSettings(false)}>×</button></div>
        <label>登入來源<select aria-label="登入來源" value={settings.browserSource} onChange={(e) => void changeBrowserSource(e.target.value as BrowserSource)}>
          <option value="none">不使用登入</option><option value="cookiesFile">cookies.txt（建議 Instagram 使用）</option><option value="chrome">Google Chrome</option><option value="edge">Microsoft Edge</option>
        </select><small>Chrome／Edge 在新版 Windows 上可能因 DPAPI 無法解密；cookies.txt 較穩定。</small></label>
        {settings.browserSource === "cookiesFile" && <label>Cookie 檔案
          <button className="folder-picker" onClick={() => void chooseCookieFile()}><ShieldCheck size={17} /><span>{settings.cookieFilePath?.split(/[\\/]/).at(-1) || "選擇 cookies.txt"}</span></button>
          <small>只保存檔案路徑，不複製或顯示 Cookie 內容。登出 Instagram 後 Cookie 可能失效。</small>
        </label>}
        <label>預設儲存位置<button className="folder-picker" onClick={() => void chooseDirectory()}><FolderOpen size={17} /><span>{settings.outputDirectory || "選擇資料夾"}</span></button></label>
        <div className="engine-box"><div><b>yt-dlp（YouTube）</b><span>{engine?.ytDlpVersion || "偵測中…"}</span></div><div><b>gallery-dl（Instagram）</b><span>{engine?.galleryDlVersion || "偵測中…"}</span></div><div><b>FFmpeg</b><span>{engine?.ffmpegVersion || "偵測中…"}</span></div><button disabled={updating} onClick={() => void updateEngine()}>{updating ? <LoaderCircle className="spin" /> : <RefreshCw />}檢查 YouTube 引擎更新</button></div>
        <button className="primary full" onClick={() => setShowSettings(false)}>完成</button>
      </aside></div>}

      {showNotice && <div className="overlay"><aside className="modal notice"><ShieldCheck size={36} /><span>BEFORE YOU START</span><h2>尊重內容，也保護自己。</h2><p>Omni Fetch 僅供下載你擁有、已獲授權，或平台允許下載的內容。請遵守著作權、服務條款與所在地法律。</p><button className="primary full" onClick={() => void acceptNotice()}>我了解並同意</button></aside></div>}
    </div>
  );
}
