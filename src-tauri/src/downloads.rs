use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures::StreamExt;
use reqwest::header::{ACCEPT_RANGES, CONTENT_LENGTH, RANGE};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{fs, io::AsyncWriteExt, sync::Mutex};
use tokio_util::sync::CancellationToken;
use url::Url;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadStatus {
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
    Canceled,
    External,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadKind {
    Http,
    Magnet,
    Torrent,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedLimits {
    pub download_bps: Option<u64>,
    pub upload_bps: Option<u64>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDownloadPayload {
    url: String,
    file_name: Option<String>,
    directory: Option<String>,
    kind: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadInfo {
    pub id: String,
    pub url: String,
    pub file_name: String,
    pub save_path: String,
    pub temp_path: String,
    pub status: DownloadStatus,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub speed_bps: u64,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub resume_supported: bool,
    pub kind: DownloadKind,
}

struct DownloadRuntime {
    info: DownloadInfo,
    cancel: CancellationToken,
}

struct DownloadManagerInner {
    downloads: Mutex<HashMap<String, DownloadRuntime>>,
    speed_limits: Mutex<SpeedLimits>,
    client: reqwest::Client,
}

#[derive(Clone)]
pub struct DownloadManager {
    inner: Arc<DownloadManagerInner>,
}

impl DownloadManager {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("FreeDownloadManager/1.0")
            .build()
            .expect("failed to build http client");
        Self {
            inner: Arc::new(DownloadManagerInner {
                downloads: Mutex::new(HashMap::new()),
                speed_limits: Mutex::new(SpeedLimits {
                    download_bps: None,
                    upload_bps: None,
                }),
                client,
            }),
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn sanitize_file_name(input: &str) -> String {
    let trimmed = input
        .trim()
        .replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "-");
    if trimmed.is_empty() {
        "download".to_string()
    } else {
        trimmed
    }
}

fn file_name_from_url(url: &Url) -> String {
    url.path_segments()
        .and_then(|segments| segments.last())
        .filter(|segment| !segment.is_empty())
        .map(sanitize_file_name)
        .unwrap_or_else(|| "download".to_string())
}

async fn ensure_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .await
        .map_err(|error| format!("Failed to create directory: {error}"))
}

fn build_unique_path(directory: &Path, file_name: &str) -> PathBuf {
    let mut candidate = directory.join(file_name);
    if !candidate.exists() {
        return candidate;
    }
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("download");
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();

    for index in 1..=9999 {
        let next = directory.join(format!("{stem} ({index}){extension}"));
        if !next.exists() {
            candidate = next;
            break;
        }
    }
    candidate
}

async fn resolve_download_directory(
    app: &AppHandle,
    directory: Option<String>,
) -> Result<PathBuf, String> {
    if let Some(dir) = directory {
        let path = PathBuf::from(dir);
        ensure_dir(&path).await?;
        return Ok(path);
    }

    if let Ok(path) = app.path().download_dir() {
        let resolved = path.to_path_buf();
        ensure_dir(&resolved).await?;
        return Ok(resolved);
    }

    if let Ok(path) = app.path().home_dir() {
        let download_dir = path.to_path_buf().join("Downloads");
        ensure_dir(&download_dir).await?;
        return Ok(download_dir);
    }

    Err("Unable to resolve a download directory".to_string())
}

async fn update_download_info(
    manager: &DownloadManager,
    id: &str,
    updater: impl FnOnce(&mut DownloadInfo),
) {
    let mut downloads = manager.inner.downloads.lock().await;
    if let Some(download) = downloads.get_mut(id) {
        updater(&mut download.info);
        download.info.updated_at = now_ms();
    }
}

async fn read_download_info(manager: &DownloadManager, id: &str) -> Option<DownloadInfo> {
    let downloads = manager.inner.downloads.lock().await;
    downloads.get(id).map(|download| download.info.clone())
}

fn parse_kind(kind: Option<String>, url: &str) -> DownloadKind {
    if let Some(kind) = kind {
        return match kind.as_str() {
            "magnet" => DownloadKind::Magnet,
            "torrent" => DownloadKind::Torrent,
            _ => DownloadKind::Http,
        };
    }

    let trimmed = url.trim().to_lowercase();
    if trimmed.starts_with("magnet:") {
        DownloadKind::Magnet
    } else if trimmed.ends_with(".torrent") {
        DownloadKind::Torrent
    } else {
        DownloadKind::Http
    }
}

#[tauri::command]
pub async fn list_downloads(state: State<'_, DownloadManager>) -> Result<Vec<DownloadInfo>, String> {
    let downloads = state.inner.downloads.lock().await;
    Ok(downloads.values().map(|entry| entry.info.clone()).collect())
}

#[tauri::command]
pub async fn set_speed_limits(
    state: State<'_, DownloadManager>,
    limits: SpeedLimits,
) -> Result<SpeedLimits, String> {
    let mut speed_limits = state.inner.speed_limits.lock().await;
    speed_limits.download_bps = limits.download_bps.filter(|value| *value > 0);
    speed_limits.upload_bps = limits.upload_bps.filter(|value| *value > 0);
    Ok(speed_limits.clone())
}

#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    state: State<'_, DownloadManager>,
    payload: StartDownloadPayload,
) -> Result<DownloadInfo, String> {
    let StartDownloadPayload {
        url,
        file_name,
        directory,
        kind,
    } = payload;
    let kind = parse_kind(kind, &url);
    let created_at = now_ms();

    if kind == DownloadKind::Http {
        let parsed = Url::parse(&url).map_err(|_| "Invalid URL".to_string())?;
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            return Err("Only http and https URLs are supported.".to_string());
        }

        let download_dir = resolve_download_directory(&app, directory).await?;
        let safe_name = file_name
            .as_deref()
            .map(sanitize_file_name)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| file_name_from_url(&parsed));
        let final_path = build_unique_path(&download_dir, &safe_name);
        let temp_extension = final_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!("{value}.part"))
            .unwrap_or_else(|| "part".to_string());
        let temp_path = final_path.with_extension(temp_extension);

        let id = uuid::Uuid::new_v4().to_string();
        let info = DownloadInfo {
            id: id.clone(),
            url: url.clone(),
            file_name: final_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(&safe_name)
                .to_string(),
            save_path: final_path.display().to_string(),
            temp_path: temp_path.display().to_string(),
            status: DownloadStatus::Queued,
            total_bytes: None,
            downloaded_bytes: 0,
            speed_bps: 0,
            error: None,
            created_at,
            updated_at: created_at,
            resume_supported: true,
            kind,
        };

        let cancel = CancellationToken::new();
        let mut downloads = state.inner.downloads.lock().await;
        downloads.insert(id.clone(), DownloadRuntime { info: info.clone(), cancel });
        drop(downloads);

        let manager = state.inner().clone();
        tauri::async_runtime::spawn(async move {
            run_download(manager, app, id).await;
        });

        return Ok(info);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let info = DownloadInfo {
        id: id.clone(),
        url: url.clone(),
        file_name: file_name
            .as_deref()
            .map(sanitize_file_name)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "External Transfer".to_string()),
        save_path: "".to_string(),
        temp_path: "".to_string(),
        status: DownloadStatus::External,
        total_bytes: None,
        downloaded_bytes: 0,
        speed_bps: 0,
        error: None,
        created_at,
        updated_at: created_at,
        resume_supported: false,
        kind,
    };

    let cancel = CancellationToken::new();
    let mut downloads = state.inner.downloads.lock().await;
    downloads.insert(id.clone(), DownloadRuntime { info: info.clone(), cancel });
    Ok(info)
}

#[tauri::command]
pub async fn pause_download(
    state: State<'_, DownloadManager>,
    id: String,
) -> Result<DownloadInfo, String> {
    let mut downloads = state.inner.downloads.lock().await;
    let Some(download) = downloads.get_mut(&id) else {
        return Err("Download not found".to_string());
    };

    if download.info.status != DownloadStatus::Running {
        return Ok(download.info.clone());
    }

    download.info.status = DownloadStatus::Paused;
    download.info.updated_at = now_ms();
    download.cancel.cancel();
    Ok(download.info.clone())
}

#[tauri::command]
pub async fn resume_download(
    app: AppHandle,
    state: State<'_, DownloadManager>,
    id: String,
) -> Result<DownloadInfo, String> {
    let mut downloads = state.inner.downloads.lock().await;
    let Some(download) = downloads.get_mut(&id) else {
        return Err("Download not found".to_string());
    };

    if download.info.kind != DownloadKind::Http {
        return Err("Resume is only available for HTTP downloads.".to_string());
    }

    if download.info.status == DownloadStatus::Completed {
        return Ok(download.info.clone());
    }

    if !download.info.resume_supported && download.info.downloaded_bytes > 0 {
        return Err("Server does not support resume. Restart the download instead.".to_string());
    }

    download.cancel = CancellationToken::new();
    download.info.status = DownloadStatus::Queued;
    download.info.error = None;
    download.info.updated_at = now_ms();
    let info = download.info.clone();
    drop(downloads);

    let manager = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        run_download(manager, app, id).await;
    });

    Ok(info)
}

#[tauri::command]
pub async fn cancel_download(
    state: State<'_, DownloadManager>,
    id: String,
) -> Result<DownloadInfo, String> {
    let mut downloads = state.inner.downloads.lock().await;
    let Some(download) = downloads.get_mut(&id) else {
        return Err("Download not found".to_string());
    };

    if matches!(download.info.status, DownloadStatus::Completed | DownloadStatus::Canceled) {
        return Ok(download.info.clone());
    }

    download.info.status = DownloadStatus::Canceled;
    download.info.updated_at = now_ms();
    download.cancel.cancel();
    Ok(download.info.clone())
}

#[tauri::command]
pub async fn restart_download(
    app: AppHandle,
    state: State<'_, DownloadManager>,
    id: String,
) -> Result<DownloadInfo, String> {
    let mut downloads = state.inner.downloads.lock().await;
    let Some(download) = downloads.get_mut(&id) else {
        return Err("Download not found".to_string());
    };

    if download.info.kind != DownloadKind::Http {
        return Err("Restart is only available for HTTP downloads.".to_string());
    }

    let temp_path = PathBuf::from(download.info.temp_path.clone());
    let _ = fs::remove_file(&temp_path).await;
    download.info.downloaded_bytes = 0;
    download.info.total_bytes = None;
    download.info.speed_bps = 0;
    download.info.status = DownloadStatus::Queued;
    download.info.error = None;
    download.cancel = CancellationToken::new();
    download.info.updated_at = now_ms();
    let info = download.info.clone();
    drop(downloads);

    let manager = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        run_download(manager, app, id).await;
    });

    Ok(info)
}

#[tauri::command]
pub async fn remove_download(
    state: State<'_, DownloadManager>,
    id: String,
) -> Result<(), String> {
    let mut downloads = state.inner.downloads.lock().await;
    let status = match downloads.get(&id) {
        Some(download) => download.info.status.clone(),
        None => return Err("Download not found".to_string()),
    };

    if matches!(
        status,
        DownloadStatus::Running | DownloadStatus::Queued | DownloadStatus::Paused
    ) {
        return Err("Stop the download before removing it.".to_string());
    }

    downloads.remove(&id);
    Ok(())
}

async fn run_download(manager: DownloadManager, app: AppHandle, id: String) {
    let info = match read_download_info(&manager, &id).await {
        Some(info) => info,
        None => return,
    };

    if info.kind != DownloadKind::Http {
        return;
    }

    let url = info.url.clone();
    let save_path = PathBuf::from(info.save_path.clone());
    let temp_path = PathBuf::from(info.temp_path.clone());
    let client = manager.inner.client.clone();
    let cancel = {
        let downloads = manager.inner.downloads.lock().await;
        match downloads.get(&id) {
            Some(entry) => entry.cancel.clone(),
            None => return,
        }
    };

    if let Some(parent) = save_path.parent() {
        if ensure_dir(parent).await.is_err() {
            update_download_info(&manager, &id, |download| {
                download.status = DownloadStatus::Failed;
                download.error = Some("Unable to create download directory".to_string());
            })
            .await;
            return;
        }
    }

    update_download_info(&manager, &id, |download| {
        download.status = DownloadStatus::Running;
        download.error = None;
    })
    .await;

    let existing_bytes = match fs::metadata(&temp_path).await {
        Ok(meta) => meta.len(),
        Err(_) => 0,
    };

    let mut downloaded_bytes = info.downloaded_bytes.max(existing_bytes);
    if downloaded_bytes > existing_bytes {
        downloaded_bytes = existing_bytes;
    }

    let mut request = client.get(&url);
    if downloaded_bytes > 0 {
        request = request.header(RANGE, format!("bytes={downloaded_bytes}-"));
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            update_download_info(&manager, &id, |download| {
                download.status = DownloadStatus::Failed;
                download.error = Some(format!("Request failed: {error}"));
            })
            .await;
            return;
        }
    };

    if response.status() == StatusCode::RANGE_NOT_SATISFIABLE {
        update_download_info(&manager, &id, |download| {
            download.status = DownloadStatus::Failed;
            download.error = Some("Range not satisfiable. Restart the download.".to_string());
            download.resume_supported = false;
        })
        .await;
        return;
    }

    if downloaded_bytes > 0 && response.status() != StatusCode::PARTIAL_CONTENT {
        update_download_info(&manager, &id, |download| {
            download.status = DownloadStatus::Failed;
            download.error = Some("Server does not support resume".to_string());
            download.resume_supported = false;
        })
        .await;
        return;
    }

    if !response.status().is_success() {
        update_download_info(&manager, &id, |download| {
            download.status = DownloadStatus::Failed;
            download.error = Some(format!("Download failed: {}", response.status()));
        })
        .await;
        return;
    }

    let content_length = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());

    let total_bytes = content_length.map(|length| length + downloaded_bytes);
    let resume_supported = response
        .headers()
        .get(ACCEPT_RANGES)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.contains("bytes"))
        .unwrap_or(downloaded_bytes > 0);

    update_download_info(&manager, &id, |download| {
        download.total_bytes = total_bytes;
        download.resume_supported = resume_supported;
    })
    .await;

    let file = if downloaded_bytes > 0 {
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&temp_path)
            .await
    } else {
        fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp_path)
            .await
    };

    let mut file = match file {
        Ok(file) => file,
        Err(error) => {
            update_download_info(&manager, &id, |download| {
                download.status = DownloadStatus::Failed;
                download.error = Some(format!("Unable to write file: {error}"));
            })
            .await;
            return;
        }
    };

    let mut stream = response.bytes_stream();
    let mut last_tick = Instant::now();
    let mut last_bytes = downloaded_bytes;
    let mut window_start = Instant::now();
    let mut window_bytes: u64 = 0;

    while let Some(chunk) = stream.next().await {
        if cancel.is_cancelled() {
            update_download_info(&manager, &id, |download| {
                if download.status != DownloadStatus::Canceled {
                    download.status = DownloadStatus::Paused;
                }
            })
            .await;
            return;
        }

        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                update_download_info(&manager, &id, |download| {
                    download.status = DownloadStatus::Failed;
                    download.error = Some(format!("Stream error: {error}"));
                })
                .await;
                return;
            }
        };

        let limit = {
            let limits = manager.inner.speed_limits.lock().await;
            limits.download_bps.unwrap_or(0)
        };

        if limit > 0 {
            let elapsed = window_start.elapsed().as_secs_f64();
            let projected = (window_bytes + chunk.len() as u64) as f64 / limit as f64;
            if projected > elapsed {
                let delay = projected - elapsed;
                tokio::time::sleep(Duration::from_secs_f64(delay.min(1.5))).await;
            }
            if window_start.elapsed() >= Duration::from_secs(1) {
                window_start = Instant::now();
                window_bytes = 0;
            }
        }

        if let Err(error) = file.write_all(&chunk).await {
            update_download_info(&manager, &id, |download| {
                download.status = DownloadStatus::Failed;
                download.error = Some(format!("Write error: {error}"));
            })
            .await;
            return;
        }

        downloaded_bytes += chunk.len() as u64;
        window_bytes += chunk.len() as u64;

        if last_tick.elapsed() >= Duration::from_millis(500) {
            let elapsed = last_tick.elapsed().as_secs_f64().max(0.1);
            let speed = ((downloaded_bytes - last_bytes) as f64 / elapsed) as u64;
            last_tick = Instant::now();
            last_bytes = downloaded_bytes;
            update_download_info(&manager, &id, |download| {
                download.downloaded_bytes = downloaded_bytes;
                download.speed_bps = speed;
            })
            .await;
        }
    }

    if let Err(error) = file.flush().await {
        update_download_info(&manager, &id, |download| {
            download.status = DownloadStatus::Failed;
            download.error = Some(format!("Flush error: {error}"));
        })
        .await;
        return;
    }

    update_download_info(&manager, &id, |download| {
        download.downloaded_bytes = downloaded_bytes;
    })
    .await;

    if let Some(parent) = save_path.parent() {
        if ensure_dir(parent).await.is_err() {
            update_download_info(&manager, &id, |download| {
                download.status = DownloadStatus::Failed;
                download.error = Some("Unable to finalize download".to_string());
            })
            .await;
            return;
        }
    }

    if let Err(error) = fs::rename(&temp_path, &save_path).await {
        update_download_info(&manager, &id, |download| {
            download.status = DownloadStatus::Failed;
            download.error = Some(format!("Finalize error: {error}"));
        })
        .await;
        return;
    }

    update_download_info(&manager, &id, |download| {
        download.status = DownloadStatus::Completed;
        download.total_bytes = download.total_bytes.or(Some(downloaded_bytes));
        download.speed_bps = 0;
    })
    .await;

    let _ = app.emit("download:completed", &id);
}
