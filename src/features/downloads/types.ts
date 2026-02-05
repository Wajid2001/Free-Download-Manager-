export type DownloadStatus =
  | "queued"
  | "running"
  | "paused"
  | "completed"
  | "failed"
  | "canceled"
  | "external"

export type DownloadKind = "http" | "magnet" | "torrent"

export type SpeedLimits = {
  downloadBps?: number | null
  uploadBps?: number | null
}

export type DownloadInfo = {
  id: string
  url: string
  fileName: string
  savePath: string
  tempPath: string
  status: DownloadStatus
  totalBytes?: number | null
  downloadedBytes: number
  speedBps: number
  error?: string | null
  createdAt: number
  updatedAt: number
  resumeSupported: boolean
  kind: DownloadKind
}
