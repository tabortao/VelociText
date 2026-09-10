import { useState, useEffect, useRef, useCallback } from "react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import {
  UploadIcon,
  FileAudioIcon,
  FileVideoIcon,
  PlusIcon,
  Trash2Icon,
  XIcon,
  AudioLinesIcon,
  CheckCircleIcon,
  AlertCircleIcon,
  DownloadIcon,
} from "lucide-react"
import { invoke } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { open } from "@tauri-apps/plugin-dialog"
import { useAppContext } from "@/lib/app-context"

const MEDIA_EXTS = [
  "wav", "mp3", "flac", "ogg", "aac", "m4a", "aiff", "caf",
  "mp4", "mkv", "webm", "mov", "avi", "flv",
]

const VIDEO_EXTS = ["mp4", "avi", "mov", "mkv", "flv", "webm"]

const DEFAULT_FORMATS = ["mp3", "wav", "flac", "m4a", "ogg", "opus", "wma"]

type ItemStatus = "pending" | "processing" | "done" | "failed" | "cancelled"

interface ConvertItem {
  path: string
  fileName: string
  status: ItemStatus
  progress: number
  error: string | null
}

interface ConvertProgressEvent {
  path: string
  stage: string
  percentage: number
  message: string
}

interface FfmpegDownloadEvent {
  stage: string
  percentage: number
  message: string
}

const isMediaFile = (p: string) =>
  MEDIA_EXTS.includes(p.split(".").pop()?.toLowerCase() ?? "")

function getFileIcon(fileName: string) {
  const ext = fileName.split(".").pop()?.toLowerCase() ?? ""
  return VIDEO_EXTS.includes(ext) ? (
    <FileVideoIcon className="size-4 text-blue-500 shrink-0" />
  ) : (
    <FileAudioIcon className="size-4 text-green-500 shrink-0" />
  )
}

export function AudioConvertPage() {
  const { t } = useAppContext()
  // null = checking, true = ready, false = missing
  const [ffmpegReady, setFfmpegReady] = useState<boolean | null>(null)
  const [ffmpegDownloading, setFfmpegDownloading] = useState(false)
  const [dlProgress, setDlProgress] = useState<FfmpegDownloadEvent | null>(null)
  const [formats, setFormats] = useState<string[]>(DEFAULT_FORMATS)
  const [format, setFormat] = useState("mp3")
  const [items, setItems] = useState<ConvertItem[]>([])
  const [batchRunning, setBatchRunning] = useState(false)
  const [isDragOver, setIsDragOver] = useState(false)
  const [flashMessage, setFlashMessage] = useState("")
  const [currentIdx, setCurrentIdx] = useState(-1)

  const flashTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const stopRef = useRef(false)
  const itemsRef = useRef<ConvertItem[]>([])

  useEffect(() => {
    itemsRef.current = items
  }, [items])

  const showFlash = useCallback((msg: string, durationMs = 3000) => {
    if (flashTimerRef.current) clearTimeout(flashTimerRef.current)
    setFlashMessage(msg)
    flashTimerRef.current = setTimeout(() => setFlashMessage(""), durationMs)
  }, [])

  // ── FFmpeg detection & format list ─────────────────────────────────────
  useEffect(() => {
    const check = async () => {
      try {
        await invoke<string>("check_ffmpeg")
        setFfmpegReady(true)
      } catch {
        setFfmpegReady(false)
      }
    }
    check()
    invoke<string[]>("get_audio_formats")
      .then((list) => {
        if (list.length > 0) setFormats(list)
      })
      .catch(() => {
        // keep the default list
      })
  }, [])

  // ── FFmpeg download progress ───────────────────────────────────────────
  useEffect(() => {
    let unlisten: UnlistenFn | null = null
    const setup = async () => {
      unlisten = await listen<FfmpegDownloadEvent>("ffmpeg-download-progress", (event) => {
        const payload = event.payload
        if (payload.stage === "completed") {
          setFfmpegDownloading(false)
          setDlProgress(null)
          setFfmpegReady(true)
        } else if (payload.stage === "error") {
          setFfmpegDownloading(false)
          setDlProgress(null)
        } else {
          setDlProgress(payload)
        }
      })
    }
    setup()

    return () => {
      unlisten?.()
    }
  }, [])

  // ── Drag & drop ─────────────────────────────────────────────────────────
  useEffect(() => {
    let unlistenDrop: UnlistenFn | null = null
    let unlistenHover: UnlistenFn | null = null

    const setup = async () => {
      unlistenDrop = await listen<string>("tauri://file-drop", (event) => {
        try {
          const paths: string[] = JSON.parse(event.payload)
          addFiles(paths.filter(isMediaFile))
        } catch { /* ignore parse errors */ }
        setIsDragOver(false)
      })

      unlistenHover = await listen<boolean>("tauri://file-drop-hover", (event) => {
        setIsDragOver(event.payload)
      })
    }

    setup()

    return () => {
      unlistenDrop?.()
      unlistenHover?.()
    }
  }, [])

  // ── File selection ──────────────────────────────────────────────────────
  const addFiles = (paths: string[]) => {
    if (batchRunning) return
    setItems((prev) => {
      const existing = new Set(prev.map((it) => it.path))
      const fresh = paths
        .filter((p) => !existing.has(p))
        .map((p) => ({
          path: p,
          fileName: p.split(/[\\/]/).pop() || p,
          status: "pending" as const,
          progress: 0,
          error: null,
        }))
      return [...prev, ...fresh]
    })
  }

  const openFileDialog = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: "Audio/Video",
            extensions: MEDIA_EXTS,
          },
        ],
      })

      if (selected) {
        if (Array.isArray(selected)) {
          addFiles(selected)
        } else if (typeof selected === "string") {
          addFiles([selected])
        }
      }
    } catch (err) {
      console.error("File dialog error:", err)
    }
  }

  const removeItem = (idx: number) => {
    if (batchRunning) return
    setItems((prev) => prev.filter((_, i) => i !== idx))
  }

  const handleClear = () => {
    if (batchRunning) return
    setItems([])
    showFlash("")
  }

  const updateItem = (idx: number, patch: Partial<ConvertItem>) => {
    setItems((prev) => prev.map((it, i) => (i === idx ? { ...it, ...patch } : it)))
  }

  // ── Batch conversion ───────────────────────────────────────────────────
  const startConvert = async () => {
    if (itemsRef.current.length === 0) {
      showFlash(t("audioConvert.noFiles"))
      return
    }

    // Reset every item and capture a stable working list
    const list = itemsRef.current.map((it) => ({
      ...it,
      status: "pending" as const,
      progress: 0,
      error: null,
    }))
    setItems(list)
    itemsRef.current = list

    stopRef.current = false
    setBatchRunning(true)
    const batchStart = Date.now()
    let succeeded = 0
    let failed = 0

    // Live per-file progress from the backend
    const unlisten = await listen<ConvertProgressEvent>("audio-convert-progress", (event) => {
      const { path, percentage } = event.payload
      if (percentage < 0) return
      setItems((prev) =>
        prev.map((it) => (it.path === path ? { ...it, progress: percentage } : it)),
      )
    })

    for (let i = 0; i < list.length; i++) {
      if (stopRef.current) break

      const item = list[i]
      setCurrentIdx(i)
      updateItem(i, { status: "processing", progress: 0 })

      try {
        await invoke<string>("convert_to_audio", { path: item.path, format })
        updateItem(i, { status: "done", progress: 100 })
        succeeded++
      } catch (err) {
        const msg = String(err instanceof Error ? err.message : err)
        if (msg === "cancelled" || stopRef.current) {
          updateItem(i, { status: "cancelled" })
          break
        }
        updateItem(i, { status: "failed", error: msg })
        failed++
      }
    }

    unlisten()
    setCurrentIdx(-1)
    setBatchRunning(false)

    if (!stopRef.current) {
      const time = ((Date.now() - batchStart) / 1000).toFixed(1)
      showFlash(
        t("audioConvert.completedToast", {
          succeeded: String(succeeded),
          failed: String(failed),
          time,
        }),
        5000,
      )
    }
  }

  const handleStop = async () => {
    stopRef.current = true
    try {
      await invoke("cancel_audio_convert")
    } catch {
      // ignore — nothing running
    }
  }

  const handleDownloadFfmpeg = async () => {
    setFfmpegDownloading(true)
    setDlProgress(null)
    try {
      await invoke<string>("download_ffmpeg")
    } catch (err) {
      console.error("FFmpeg download failed:", err)
      setFfmpegDownloading(false)
    }
  }

  // ── Render helpers ──────────────────────────────────────────────────────
  const doneCount = items.filter((it) => it.status === "done").length
  const overallPercent =
    items.length > 0 ? Math.round((doneCount / items.length) * 100) : 0

  const statusBadge = (item: ConvertItem) => {
    switch (item.status) {
      case "done":
        return (
          <Badge variant="outline" className="gap-1 border-green-300 dark:border-green-700">
            <CheckCircleIcon className="size-3 text-green-600 dark:text-green-400" />
            <span className="text-green-700 dark:text-green-300">{t("audioConvert.done")}</span>
          </Badge>
        )
      case "failed":
        return (
          <Badge variant="destructive" className="gap-1">
            <AlertCircleIcon className="size-3" />
            {t("audioConvert.failed")}
          </Badge>
        )
      case "cancelled":
        return <Badge variant="outline">{t("audioConvert.cancelled")}</Badge>
      case "processing":
        return (
          <span className="text-xs text-muted-foreground tabular-nums">
            {item.progress > 0
              ? t("audioConvert.processing", { percent: String(Math.floor(item.progress)) })
              : t("audioConvert.processingIndeterminate")}
          </span>
        )
      default:
        return <span className="text-xs text-muted-foreground">{t("audioConvert.pending")}</span>
    }
  }

  // ── Render ──────────────────────────────────────────────────────────────
  return (
    <div className="px-4 lg:px-6 space-y-4">
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="flex items-center gap-2">
            <AudioLinesIcon className="size-5" />
            {t("audioConvert.title")}
          </CardTitle>
          <CardDescription>{t("audioConvert.desc")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {/* Flash message toast */}
          {flashMessage && (
            <div className="fixed bottom-6 right-6 z-50 px-4 py-2 bg-foreground text-background text-sm rounded-lg shadow-lg animate-in fade-in slide-in-from-bottom-2">
              {flashMessage}
            </div>
          )}

          {/* Checking FFmpeg */}
          {ffmpegReady === null && (
            <p className="text-sm text-muted-foreground py-6 text-center">
              {t("audioConvert.checkingFfmpeg")}
            </p>
          )}

          {/* FFmpeg missing */}
          {ffmpegReady === false && (
            <div className="py-6 text-center space-y-3">
              <p className="text-destructive font-medium">{t("audioConvert.ffmpegMissing")}</p>
              {ffmpegDownloading && dlProgress && (
                <p className="text-sm text-muted-foreground">
                  {dlProgress.stage === "extracting"
                    ? t("settings.ffmpegExtracting")
                    : `${t("settings.ffmpegDownloading")} ${dlProgress.percentage.toFixed(0)}%`}
                </p>
              )}
              <Button size="sm" onClick={handleDownloadFfmpeg} disabled={ffmpegDownloading}>
                <DownloadIcon className="size-3.5 mr-1" />
                {t("settings.ffmpegDownload")}
              </Button>
            </div>
          )}

          {ffmpegReady && (
            <>
              {/* Output format selector */}
              <div className="flex items-center gap-2">
                <span className="text-sm text-muted-foreground shrink-0">
                  {t("audioConvert.formatLabel")}
                </span>
                <Select value={format} onValueChange={setFormat} disabled={batchRunning}>
                  <SelectTrigger className="w-[110px]">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {formats.map((f) => (
                      <SelectItem key={f} value={f}>
                        {f.toUpperCase()}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              {/* Drop zone */}
              {items.length === 0 ? (
                <div
                  className={`flex flex-col items-center justify-center border-2 border-dashed rounded-lg p-12 cursor-pointer transition-colors ${
                    isDragOver
                      ? "border-primary bg-primary/5"
                      : "border-muted-foreground/25 hover:border-primary/50"
                  }`}
                  onClick={() => !batchRunning && openFileDialog()}
                >
                  <div className="flex flex-col items-center gap-3">
                    <UploadIcon
                      className={`size-10 ${isDragOver ? "text-primary" : "text-muted-foreground"}`}
                    />
                    <p className="text-sm text-muted-foreground">
                      {isDragOver ? t("audioConvert.dropFiles") : t("audioConvert.clickOrDrag")}
                    </p>
                    <Button variant="secondary" size="sm">
                      {t("audioConvert.selectFiles")}
                    </Button>
                  </div>
                </div>
              ) : (
                <>
                  {/* Overall progress */}
                  {batchRunning && (
                    <div className="space-y-1.5">
                      <div className="flex items-center justify-between text-xs text-muted-foreground">
                        <span>
                          {t("audioConvert.fileCount", { count: String(currentIdx + 1) })} /{" "}
                          {t("audioConvert.fileCount", { count: String(items.length) })}
                        </span>
                        <span className="tabular-nums">{overallPercent}%</span>
                      </div>
                      <div className="h-2 bg-muted rounded-full overflow-hidden">
                        <div
                          className="h-full bg-primary transition-all duration-300 ease-out rounded-full"
                          style={{ width: `${Math.max(overallPercent, 2)}%` }}
                        />
                      </div>
                    </div>
                  )}

                  {/* File list — scrolls internally so action buttons stay visible */}
                  <div className="border rounded-md max-h-[50vh] overflow-y-auto">
                    <table className="w-full text-sm">
                      <thead className="bg-muted/50 sticky top-0">
                        <tr>
                          <th className="text-left px-3 py-2">{t("audioConvert.file")}</th>
                          <th className="text-left px-3 py-2 w-[140px]">{t("audioConvert.status")}</th>
                          <th className="text-right px-3 py-2 w-[100px]"></th>
                        </tr>
                      </thead>
                      <tbody>
                        {items.map((item, i) => (
                          <tr
                            key={item.path}
                            className={`border-t border-border transition-colors ${
                              i === currentIdx && batchRunning ? "bg-accent/50" : "hover:bg-accent/30"
                            }`}
                          >
                            <td className="px-3 py-1.5">
                              <div className="flex items-center gap-2 min-w-0">
                                {getFileIcon(item.fileName)}
                                <span className="truncate" title={item.path}>
                                  {item.fileName}
                                </span>
                              </div>
                              {item.error && (
                                <p className="text-xs text-destructive mt-0.5 break-all whitespace-pre-wrap" title={item.error}>
                                  {item.error}
                                </p>
                              )}
                            </td>
                            <td className="px-3 py-1.5">
                              {statusBadge(item)}
                            </td>
                            <td className="px-3 py-1.5 text-right">
                              {!batchRunning && (
                                <button
                                  className="p-1 rounded hover:bg-muted transition-colors text-muted-foreground hover:text-foreground"
                                  title={t("audioConvert.remove")}
                                  onClick={() => removeItem(i)}
                                >
                                  <XIcon className="size-3.5" />
                                </button>
                              )}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>

                  {/* Actions */}
                  <div className="flex items-center justify-between flex-wrap gap-2">
                    <div className="flex items-center gap-2">
                      {batchRunning ? (
                        <Button variant="destructive" size="sm" onClick={handleStop}>
                          <XIcon className="size-3.5 mr-1" />
                          {t("audioConvert.stop")}
                        </Button>
                      ) : (
                        <>
                          <Button size="sm" onClick={startConvert}>
                            <AudioLinesIcon className="size-3.5 mr-1" />
                            {t("audioConvert.start")}
                          </Button>
                          <Button variant="outline" size="sm" onClick={() => openFileDialog()}>
                            <PlusIcon className="size-3.5 mr-1" />
                            {t("audioConvert.addMore")}
                          </Button>
                          <Button variant="ghost" size="sm" onClick={handleClear}>
                            <Trash2Icon className="size-3.5 mr-1" />
                            {t("audioConvert.clear")}
                          </Button>
                        </>
                      )}
                    </div>
                    <span className="text-xs text-muted-foreground">
                      {t("audioConvert.fileCount", { count: String(items.length) })}
                    </span>
                  </div>
                </>
              )}
            </>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
