import { invoke } from "@tauri-apps/api/core"
import type { DownloadInfo, SpeedLimits, DownloadKind } from "@/features/downloads/types"

export const listDownloads = () => invoke<DownloadInfo[]>("list_downloads")

export const startDownload = (payload: {
  url: string
  fileName?: string
  directory?: string
  kind?: DownloadKind
}) => invoke<DownloadInfo>("start_download", { payload })

export const pauseDownload = (id: string) => invoke<DownloadInfo>("pause_download", { id })

export const resumeDownload = (id: string) => invoke<DownloadInfo>("resume_download", { id })

export const cancelDownload = (id: string) => invoke<DownloadInfo>("cancel_download", { id })

export const restartDownload = (id: string) => invoke<DownloadInfo>("restart_download", { id })

export const removeDownload = (id: string) => invoke<void>("remove_download", { id })

export const setSpeedLimits = (limits: SpeedLimits) =>
  invoke<SpeedLimits>("set_speed_limits", { limits })
