# Omni Fetch

Omni Fetch 是一個 Windows 10/11 x64 桌面下載器，以 Rust、Tauri 2、React 與 TypeScript 製作。介面可切換 Instagram／YouTube／一般網頁模式；Instagram 由 gallery-dl 處理圖片、輪播與 Reels，YouTube 與一般網頁由 yt-dlp 處理影片、HLS（`.m3u8`）、MP4 與 MP3。

> 僅可下載你擁有、已獲授權或平台允許取得的內容。Omni Fetch 不繞過 DRM、付費牆或平台存取控制。

## 開發環境

- Rust 1.88 或更新版本，MSVC target
- Node.js 22 或更新版本
- pnpm 11
- Tauri 2 的 Windows prerequisites（WebView2 與 Microsoft C++ Build Tools）

```powershell
pnpm install
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
```

若 PATH 已有 `yt-dlp` 與 `ffmpeg`，開發模式可直接使用。若要準備可封裝且已驗證的 sidecar：

```powershell
.\scripts\prepare-tools.ps1
pnpm tauri build
```

`prepare-tools.ps1` 只接受 HTTPS 上游檔案，並依官方校驗清單驗證 SHA-256。產生的執行檔位於 `src-tauri/binaries` 且不納入版本控制。

## 架構

- React 僅呼叫 `src/api.ts` 中的 Tauri commands，不具有任意 shell 執行權限。
- Rust 驗證平台、URL、格式、畫質與輸出路徑後才生成 yt-dlp 參數。
- 單一背景 worker 依序執行任務；狀態透過 `download://progress` 與 `download://state-changed` 推送。
- 下載時顯示即時百分比、速度與剩餘時間；下載引擎尚未提供可量測進度時會顯示持續動畫。
- 可選擇不登入、Chrome、Edge，或由瀏覽器擴充功能匯出的 Netscape `cookies.txt`。
- 一般網頁模式接受影片頁面或直接的 `.m3u8`；若 CDN 驗證 Referer，可在「來源頁網址」填入實際播放影片的網頁。
- 已知的 `18porn.cc/video/{id}.html` 網址會先自動轉成對應 CDN `.m3u8`，並自動補上原影片頁 Referer；不符合規則時才交由通用解析器處理。
- 一般網頁分析完成後可自訂儲存檔名；副檔名依 MP4、MP3 或原始格式自動決定。
- App 使用 `cookies.txt` 時只保存檔案路徑，不複製、顯示或記錄 Cookie 內容。
- 使用者更新的 yt-dlp 儲存在 App Local Data；內建引擎永遠保留為回退版本。

## 第一版限制

- 不支援媒體播放清單批次、直播、Instagram Stories、帳號整批下載或跨平台安裝包。
- 一般網頁模式不攔截瀏覽器 Network；yt-dlp 無法辨識頁面時，需自行提供 `.m3u8`。單一 `.ts` 片段不代表完整影片。
- 不支援 DRM、付費牆或其他存取控制繞過。
- 任務與下載歷史只保留在記憶體；`.part` 檔會保留供重新加入同一 URL 時續傳。
- 新版 Windows 上 Chrome／Edge Cookie 可能因 DPAPI／App-Bound Encryption 無法解密；Instagram 建議改選 `cookies.txt`。
- Cookie 檔案等同登入憑證，請勿分享或上傳；登出 Instagram 後通常需要重新匯出。
