import type { DownloadInfo } from "@/features/downloads/types"

const units = ["B", "KB", "MB", "GB", "TB"] as const

export const formatBytes = (value?: number | null) => {
  if (value === undefined || value === null || Number.isNaN(value)) {
    return "Unknown"
  }
  if (value === 0) return "0 B"
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1)
  const normalized = value / Math.pow(1024, index)
  return `${normalized.toFixed(normalized >= 10 ? 0 : 1)} ${units[index]}`
}

export const formatSpeed = (value?: number | null) => {
  if (!value || value <= 0) return "0 B/s"
  return `${formatBytes(value)}/s`
}

export const formatEta = (downloaded: number, total?: number | null, speed?: number) => {
  if (!total || !speed || speed <= 0 || downloaded >= total) return "—"
  const seconds = Math.max(0, Math.ceil((total - downloaded) / speed))
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  const secs = seconds % 60
  if (hours > 0) return `${hours}h ${minutes}m`
  if (minutes > 0) return `${minutes}m ${secs}s`
  return `${secs}s`
}

export const formatPercent = (downloaded: number, total?: number | null) => {
  if (!total || total <= 0) return 0
  return Math.min(100, Math.max(0, (downloaded / total) * 100))
}

export const sortDownloads = (downloads: DownloadInfo[]) =>
  [...downloads].sort((a, b) => b.createdAt - a.createdAt)

export const inferKind = (input: string) => {
  const trimmed = input.trim().toLowerCase()
  if (trimmed.startsWith("magnet:")) return "magnet" as const
  if (trimmed.endsWith(".torrent")) return "torrent" as const
  return "http" as const
}

export const normalizeUrl = (input: string) => {
  const trimmed = input.trim()
  if (!trimmed) return ""
  if (trimmed.startsWith("magnet:")) return trimmed
  if (/^https?:\/\//i.test(trimmed)) return trimmed
  return `https://${trimmed}`
}

export const parseNumberInput = (value: string) => {
  if (!value) return undefined
  const parsed = Number(value)
  if (!Number.isFinite(parsed) || parsed <= 0) return undefined
  return parsed
}

export const toBps = (value?: number, unit?: string) => {
  if (!value || !unit) return undefined
  const normalized = unit.toLowerCase()
  if (normalized.startsWith("k")) return Math.round(value * 1024)
  if (normalized.startsWith("m")) return Math.round(value * 1024 * 1024)
  if (normalized.startsWith("g")) return Math.round(value * 1024 * 1024 * 1024)
  return Math.round(value)
}
