import { useCallback, useEffect, useMemo, useState } from "react"
import { openPath, openUrl } from "@tauri-apps/plugin-opener"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Separator } from "@/components/ui/separator"

import {
  cancelDownload,
  listDownloads,
  pauseDownload,
  removeDownload,
  restartDownload,
  resumeDownload,
  setSpeedLimits,
  startDownload,
} from "@/features/downloads/api"
import type { DownloadInfo, DownloadKind, DownloadStatus } from "@/features/downloads/types"
import {
  formatBytes,
  formatEta,
  formatPercent,
  formatSpeed,
  inferKind,
  normalizeUrl,
  parseNumberInput,
  sortDownloads,
  toBps,
} from "@/features/downloads/utils"

const defaultUnits = ["KB/s", "MB/s", "GB/s"]

const statusTone: Record<DownloadStatus, "default" | "secondary" | "destructive"> = {
  queued: "secondary",
  running: "default",
  paused: "secondary",
  completed: "default",
  failed: "destructive",
  canceled: "secondary",
  external: "secondary",
}

const statusLabel: Record<DownloadStatus, string> = {
  queued: "Queued",
  running: "Running",
  paused: "Paused",
  completed: "Completed",
  failed: "Failed",
  canceled: "Canceled",
  external: "External",
}

const filterOptions = [
  { value: "all", label: "All" },
  { value: "active", label: "Active" },
  { value: "completed", label: "Completed" },
  { value: "failed", label: "Failed" },
]

const isActiveStatus = (status: DownloadStatus) =>
  status === "running" || status === "queued" || status === "paused"

export function DownloadManager() {
  const [downloads, setDownloads] = useState<DownloadInfo[]>([])
  const [filter, setFilter] = useState("all")
  const [formUrl, setFormUrl] = useState("")
  const [formFileName, setFormFileName] = useState("")
  const [formDirectory, setFormDirectory] = useState("")
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [downloadLimitValue, setDownloadLimitValue] = useState("")
  const [downloadLimitUnit, setDownloadLimitUnit] = useState("MB/s")
  const [uploadLimitValue, setUploadLimitValue] = useState("")
  const [uploadLimitUnit, setUploadLimitUnit] = useState("MB/s")

  const refreshDownloads = useCallback(async () => {
    try {
      const data = await listDownloads()
      setDownloads(sortDownloads(data))
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to load downloads")
    }
  }, [])

  useEffect(() => {
    refreshDownloads()
    const handle = window.setInterval(refreshDownloads, 900)
    return () => window.clearInterval(handle)
  }, [refreshDownloads])

  const visibleDownloads = useMemo(() => {
    if (filter === "active") return downloads.filter((download) => isActiveStatus(download.status))
    if (filter === "completed")
      return downloads.filter((download) => download.status === "completed")
    if (filter === "failed") return downloads.filter((download) => download.status === "failed")
    return downloads
  }, [downloads, filter])

  const stats = useMemo(() => {
    const active = downloads.filter((download) => isActiveStatus(download.status)).length
    const completed = downloads.filter((download) => download.status === "completed").length
    const failed = downloads.filter((download) => download.status === "failed").length
    const totalSpeed = downloads
      .filter((download) => download.status === "running")
      .reduce((sum, download) => sum + download.speedBps, 0)
    return { active, completed, failed, totalSpeed }
  }, [downloads])

  const submitDownload = async (url: string, kind: DownloadKind) => {
    setIsSubmitting(true)
    try {
      await startDownload({
        url,
        fileName: formFileName || undefined,
        directory: formDirectory || undefined,
        kind,
      })
      setFormUrl("")
      setFormFileName("")
      setErrorMessage(null)
      await refreshDownloads()
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to start download")
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleStartDownload = async (kindOverride?: DownloadKind) => {
    setErrorMessage(null)
    const rawValue = formUrl.trim()
    const normalized = kindOverride === "torrent" ? rawValue : normalizeUrl(rawValue)
    const kind = kindOverride ?? inferKind(normalized)
    if (!normalized) {
      setErrorMessage("Enter a valid URL or magnet link.")
      return
    }

    if (kind === "magnet" && !normalized.startsWith("magnet:")) {
      setErrorMessage("Magnet links must start with magnet:.")
      return
    }

    if (kind === "torrent" && !normalized.toLowerCase().endsWith(".torrent")) {
      setErrorMessage("Select a .torrent file to continue.")
      return
    }

    await submitDownload(normalized, kind)
  }

  const handleAddTorrent = async () => {
    await handleStartDownload("torrent")
  }

  const handleApplyLimits = async () => {
    const downloadValue = parseNumberInput(downloadLimitValue)
    const uploadValue = parseNumberInput(uploadLimitValue)
    const downloadBps = toBps(downloadValue, downloadLimitUnit)
    const uploadBps = toBps(uploadValue, uploadLimitUnit)
    try {
      await setSpeedLimits({
        downloadBps: downloadBps ?? null,
        uploadBps: uploadBps ?? null,
      })
      setErrorMessage(null)
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to apply speed limits")
    }
  }

  const handleOpen = async (target: string) => {
    try {
      if (target.startsWith("http") || target.startsWith("magnet:")) {
        await openUrl(target)
      } else {
        await openPath(target)
      }
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Unable to open target")
    }
  }

  const handleRemove = async (download: DownloadInfo) => {
    try {
      await removeDownload(download.id)
      await refreshDownloads()
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to remove download")
    }
  }

  return (
    <div className="bg-background min-h-screen">
      <div className="mx-auto flex min-h-screen w-full max-w-6xl flex-col gap-6 px-4 pb-10 pt-8 sm:px-6 lg:px-8">
        <header className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="space-y-1">
            <p className="text-muted-foreground text-sm font-medium tracking-wide">
              Tauri Download Manager
            </p>
            <h1 className="text-2xl font-semibold tracking-tight sm:text-3xl">
              Free Download Manager
            </h1>
            <p className="text-muted-foreground max-w-2xl text-sm">
              Manage parallel downloads, throttle speeds, and keep every transfer under control.
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              onClick={() => handleStartDownload()}
              disabled={isSubmitting}
              className="min-w-35"
            >
              Add Download
            </Button>
            <Button variant="secondary" onClick={handleAddTorrent} disabled={isSubmitting}>
              Add Torrent
            </Button>
          </div>
        </header>

        {errorMessage && (
          <Card className="border-destructive/40 bg-destructive/10">
            <CardContent className="text-destructive text-sm">{errorMessage}</CardContent>
          </Card>
        )}

        <div className="grid gap-6 lg:grid-cols-[minmax(0,360px)_minmax(0,1fr)]">
          <div className="flex flex-col gap-6">
            <Card>
              <CardHeader>
                <CardTitle>New Download</CardTitle>
                <CardDescription>Add a file, magnet link, or torrent source.</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-2">
                  <label className="text-sm font-medium">URL or Magnet</label>
                  <Input
                    value={formUrl}
                    onChange={(event) => setFormUrl(event.target.value)}
                    placeholder="https://example.com/file.zip or magnet:?xt=..."
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">File name (optional)</label>
                  <Input
                    value={formFileName}
                    onChange={(event) => setFormFileName(event.target.value)}
                    placeholder="filename.zip"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Save directory (optional)</label>
                  <Input
                    value={formDirectory}
                    onChange={(event) => setFormDirectory(event.target.value)}
                    placeholder="/home/user/Downloads"
                  />
                </div>
                <div className="flex flex-col gap-2 sm:flex-row">
                  <Button onClick={() => handleStartDownload()} disabled={isSubmitting}>
                    Start Download
                  </Button>
                  <Button
                    variant="secondary"
                    onClick={() => handleStartDownload("magnet")}
                    disabled={isSubmitting}
                  >
                    Start Magnet
                  </Button>
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Speed Limits</CardTitle>
                <CardDescription>Throttle transfers to keep bandwidth balanced.</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Download limit</label>
                  <div className="flex gap-2">
                    <Input
                      value={downloadLimitValue}
                      onChange={(event) => setDownloadLimitValue(event.target.value)}
                      placeholder="Unlimited"
                      inputMode="decimal"
                    />
                    <Select value={downloadLimitUnit} onValueChange={setDownloadLimitUnit}>
                      <SelectTrigger className="w-28">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {defaultUnits.map((unit) => (
                          <SelectItem key={unit} value={unit}>
                            {unit}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Upload limit</label>
                  <div className="flex gap-2">
                    <Input
                      value={uploadLimitValue}
                      onChange={(event) => setUploadLimitValue(event.target.value)}
                      placeholder="Unlimited"
                      inputMode="decimal"
                    />
                    <Select value={uploadLimitUnit} onValueChange={setUploadLimitUnit}>
                      <SelectTrigger className="w-28">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {defaultUnits.map((unit) => (
                          <SelectItem key={unit} value={unit}>
                            {unit}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </div>
              </CardContent>
              <CardFooter className="flex items-center justify-between">
                <div className="text-muted-foreground text-xs">
                  Upload limits apply to torrent seeding.
                </div>
                <Button variant="outline" onClick={handleApplyLimits}>
                  Apply Limits
                </Button>
              </CardFooter>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Overview</CardTitle>
                <CardDescription>Live transfer metrics.</CardDescription>
              </CardHeader>
              <CardContent className="grid gap-4">
                <div className="flex items-center justify-between">
                  <span className="text-sm">Active</span>
                  <span className="text-sm font-medium">{stats.active}</span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-sm">Completed</span>
                  <span className="text-sm font-medium">{stats.completed}</span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-sm">Failed</span>
                  <span className="text-sm font-medium">{stats.failed}</span>
                </div>
                <Separator />
                <div className="flex items-center justify-between">
                  <span className="text-sm">Total speed</span>
                  <span className="text-sm font-medium">{formatSpeed(stats.totalSpeed)}</span>
                </div>
              </CardContent>
            </Card>
          </div>

          <div className="flex flex-col gap-6">
            <Card>
              <CardHeader className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <div>
                  <CardTitle>Downloads</CardTitle>
                  <CardDescription>Monitor and control every transfer.</CardDescription>
                </div>
                <div className="flex items-center gap-2">
                  <Select value={filter} onValueChange={setFilter}>
                    <SelectTrigger className="w-36">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {filterOptions.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              </CardHeader>
              <CardContent className="space-y-4">
                {visibleDownloads.length === 0 && (
                  <div className="text-muted-foreground text-sm">No downloads yet.</div>
                )}
                {visibleDownloads.map((download) => {
                  const percent = formatPercent(download.downloadedBytes, download.totalBytes)
                  const isRunning = download.status === "running"
                  const canResume =
                    download.status === "paused" ||
                    (download.status === "failed" && download.resumeSupported)
                  const canRestart = download.status === "failed" || download.status === "canceled"

                  return (
                    <div key={download.id} className="bg-muted/40 space-y-3 rounded-2xl border p-4">
                      <div className="flex flex-wrap items-start justify-between gap-3">
                        <div className="space-y-1">
                          <div className="flex flex-wrap items-center gap-2">
                            <h3 className="text-sm font-semibold">{download.fileName}</h3>
                            <Badge variant={statusTone[download.status]}>
                              {statusLabel[download.status]}
                            </Badge>
                            {download.kind !== "http" && (
                              <Badge variant="secondary">{download.kind.toUpperCase()}</Badge>
                            )}
                          </div>
                          <p className="text-muted-foreground text-xs break-all">{download.url}</p>
                        </div>
                        <div className="flex flex-wrap items-center gap-2">
                          {isRunning && (
                            <Button
                              size="sm"
                              variant="secondary"
                              onClick={() => pauseDownload(download.id).then(refreshDownloads)}
                            >
                              Pause
                            </Button>
                          )}
                          {canResume && (
                            <Button
                              size="sm"
                              onClick={() => resumeDownload(download.id).then(refreshDownloads)}
                            >
                              Resume
                            </Button>
                          )}
                          {canRestart && (
                            <Button
                              size="sm"
                              variant="secondary"
                              onClick={() => restartDownload(download.id).then(refreshDownloads)}
                            >
                              Restart
                            </Button>
                          )}
                          {download.status !== "completed" && download.status !== "external" && (
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() => cancelDownload(download.id).then(refreshDownloads)}
                            >
                              Cancel
                            </Button>
                          )}
                          {download.status === "completed" && (
                            <Button size="sm" onClick={() => handleOpen(download.savePath)}>
                              Open
                            </Button>
                          )}
                          {download.status === "external" && (
                            <Button size="sm" onClick={() => handleOpen(download.url)}>
                              Open External
                            </Button>
                          )}
                          {(download.status === "completed" ||
                            download.status === "failed" ||
                            download.status === "canceled") && (
                            <Button
                              size="sm"
                              variant="ghost"
                              onClick={() => handleRemove(download)}
                            >
                              Remove
                            </Button>
                          )}
                        </div>
                      </div>

                      <div className="space-y-2">
                        <div className="h-2 w-full overflow-hidden rounded-full bg-muted">
                          <div
                            className="bg-primary h-full transition-all"
                            style={{ width: `${percent}%` }}
                          />
                        </div>
                        <div className="text-muted-foreground flex flex-wrap items-center justify-between gap-2 text-xs">
                          <span>
                            {formatBytes(download.downloadedBytes)} /{" "}
                            {formatBytes(download.totalBytes)}
                          </span>
                          <span>{formatSpeed(download.speedBps)}</span>
                          <span>
                            ETA{" "}
                            {formatEta(
                              download.downloadedBytes,
                              download.totalBytes,
                              download.speedBps
                            )}
                          </span>
                        </div>
                      </div>

                      {download.error && (
                        <div className="text-destructive text-xs">{download.error}</div>
                      )}
                    </div>
                  )
                })}
              </CardContent>
            </Card>
          </div>
        </div>
      </div>
    </div>
  )
}
