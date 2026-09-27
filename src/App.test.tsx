import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import App from "./App";

const { analyze, enqueue } = vi.hoisted(() => ({ analyze: vi.fn(), enqueue: vi.fn() }));

vi.mock("./api", () => ({ api: {
  settings: vi.fn().mockResolvedValue({ outputDirectory: "C:\\Downloads", platformMode: "instagram", browserSource: "none", cookieFilePath: undefined, noticeAccepted: true }),
  engineInfo: vi.fn().mockResolvedValue({ ytDlpVersion: "2026.08.19", galleryDlVersion: "1.32.12-dev", ffmpegVersion: "ffmpeg 9.0.1", source: "bundled" }),
  analyze,
  enqueue,
  saveSettings: vi.fn((value) => Promise.resolve(value)),
  onTaskChanged: vi.fn().mockResolvedValue(() => undefined),
  onProgress: vi.fn().mockResolvedValue(() => undefined),
  cancel: vi.fn(), retry: vi.fn(), openFolder: vi.fn(), checkUpdate: vi.fn(), installUpdate: vi.fn(),
} }));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const media = {
  url: "https://youtu.be/demo", platform: "youtube", title: "Demo video", uploader: "Creator",
  thumbnail: undefined, durationSeconds: 20, isCarousel: false,
  items: [{ id: "demo", title: "Demo video", mediaType: "video" }],
  qualities: [{ height: 720, label: "720p" }, { height: 1080, label: "1080p" }],
  supportedOutputs: ["mp4", "mp3", "original"],
};

const webMedia = {
  url: "https://cdn.example.com/video/master.m3u8", platform: "web", title: "Web video",
  thumbnail: undefined, durationSeconds: 20, isCarousel: false,
  items: [{ id: "web-video", title: "Web video", mediaType: "video" }],
  qualities: [], supportedOutputs: ["mp4", "mp3", "original"],
};

describe("App", () => {
  beforeEach(() => { analyze.mockReset(); enqueue.mockReset(); });

  it("disables analysis until a URL is entered", async () => {
    render(<App />);
    await act(async () => undefined);
    expect(screen.getByRole("button", { name: /分析連結/ })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("媒體網址"), { target: { value: "https://youtu.be/demo" } });
    expect(screen.getByRole("button", { name: /分析連結/ })).toBeEnabled();
  });

  it("shows analyzed format and quality choices", async () => {
    analyze.mockResolvedValue(media);
    render(<App />);
    fireEvent.change(screen.getByLabelText("媒體網址"), { target: { value: media.url } });
    fireEvent.click(screen.getByRole("button", { name: /分析連結/ }));
    expect(await screen.findByText("Demo video")).toBeInTheDocument();
    expect(screen.getByText("1080p")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /加入下載佇列/ })).toBeEnabled();
  });

  it("surfaces backend analysis errors", async () => {
    analyze.mockRejectedValue("目前只支援 Instagram 與 YouTube 網址。");
    render(<App />);
    fireEvent.change(screen.getByLabelText("媒體網址"), { target: { value: "https://example.com" } });
    fireEvent.click(screen.getByRole("button", { name: /分析連結/ }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("目前只支援"));
  });

  it("offers cookies.txt as an authentication source", async () => {
    render(<App />);
    await act(async () => undefined);
    fireEvent.click(screen.getByRole("button", { name: "設定" }));
    expect(screen.getByRole("option", { name: /cookies\.txt/ })).toBeInTheDocument();
  });

  it("switches between Instagram and YouTube modes", async () => {
    render(<App />);
    await act(async () => undefined);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "YouTube" }));
    });
    expect(screen.getByPlaceholderText(/YouTube 影片/)).toBeInTheDocument();
  });

  it("shows the optional source page only in web mode", async () => {
    render(<App />);
    await act(async () => undefined);
    expect(screen.queryByLabelText("來源頁網址")).not.toBeInTheDocument();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "一般網頁" }));
    });
    expect(screen.getByPlaceholderText(/\.m3u8/)).toBeInTheDocument();
    expect(screen.getByLabelText("來源頁網址")).toBeInTheDocument();
  });

  it("passes the source page through analysis and download", async () => {
    analyze.mockResolvedValue(webMedia);
    enqueue.mockResolvedValue({
      id: "task-web", request: { outputFormat: "mp4" }, title: "Web video", platform: "web", status: "queued",
      percent: 0, outputs: [],
    });
    render(<App />);
    await act(async () => undefined);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "一般網頁" }));
    });
    fireEvent.change(screen.getByLabelText("媒體網址"), { target: { value: webMedia.url } });
    fireEvent.change(screen.getByLabelText("來源頁網址"), { target: { value: "https://example.com/watch/123" } });
    fireEvent.click(screen.getByRole("button", { name: /分析連結/ }));
    expect(await screen.findByText("Web video")).toBeInTheDocument();
    expect(screen.queryByText("影片畫質")).not.toBeInTheDocument();
    expect(screen.getByLabelText("儲存檔名")).toHaveValue("Web video");
    expect(analyze).toHaveBeenCalledWith(expect.objectContaining({
      platform: "web",
      sourcePageUrl: "https://example.com/watch/123",
    }));
    fireEvent.change(screen.getByLabelText("儲存檔名"), { target: { value: "My clip.mp4" } });
    fireEvent.click(screen.getByRole("button", { name: /加入下載佇列/ }));
    await waitFor(() => expect(enqueue).toHaveBeenCalledWith(expect.objectContaining({
      platform: "web",
      sourcePageUrl: "https://example.com/watch/123",
      outputFilename: "My clip.mp4",
      qualityHeight: undefined,
    })));
    expect(await screen.findByText(/等待下載資料/)).toBeInTheDocument();
  });
});
